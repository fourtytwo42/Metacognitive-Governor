#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Linux package UAT must run on Linux." >&2
  exit 1
fi
if [[ "$EUID" -ne 0 ]]; then
  echo "Linux package UAT requires root for dpkg install/remove operations." >&2
  exit 1
fi

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
deb="${1:-$repository_root/target/release/bundle/deb/MCG_0.1.0_amd64.deb}"
appimage="${2:-$repository_root/target/release/bundle/appimage/MCG_0.1.0_amd64.AppImage}"
uat_user="${MCG_UAT_USER:-${SUDO_USER:-$(stat -c '%U' "$repository_root")}}"

[[ -f "$deb" ]] || { echo "Missing deb package: $deb" >&2; exit 1; }
[[ -f "$appimage" ]] || { echo "Missing AppImage package: $appimage" >&2; exit 1; }
[[ "$uat_user" != "root" ]] || { echo "MCG_UAT_USER must name a non-root desktop user." >&2; exit 1; }
id "$uat_user" >/dev/null
for command in curl dpkg dpkg-deb find import pgrep python3 runuser ss stat xdotool Xvfb; do
  command -v "$command" >/dev/null || { echo "Missing UAT dependency: $command" >&2; exit 1; }
done

package_name="$(dpkg-deb -f "$deb" Package)"
if dpkg-query -W -f='${Status}' "$package_name" 2>/dev/null | grep -q 'install ok installed'; then
  echo "Linux package UAT refuses to replace an existing $package_name installation." >&2
  exit 1
fi

uat_root="$(mktemp -d /tmp/mcg-linux-package-uat-XXXXXX)"
chown "$uat_user":"$(id -gn "$uat_user")" "$uat_root"
uat_home="$uat_root/home"
installed_data="$uat_root/installed-data"
appimage_data="$uat_root/appimage-data"
mkdir -p "$uat_home" "$installed_data" "$appimage_data"
chown -R "$uat_user":"$(id -gn "$uat_user")" "$uat_home" "$installed_data" "$appimage_data"

package_installed=false
background_pids=()

stop_tree() {
  local pid="${1:-}"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 0
  local child
  while read -r child; do
    [[ -n "$child" ]] && stop_tree "$child"
  done < <(pgrep -P "$pid" 2>/dev/null || true)
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
}

cleanup() {
  local pid
  for pid in "${background_pids[@]:-}"; do stop_tree "$pid"; done
  if [[ "$package_installed" == true ]]; then
    dpkg -r "$package_name" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

wait_for_health() {
  local port="$1"
  local attempt
  for attempt in $(seq 1 100); do
    if curl --fail --silent "http://127.0.0.1:$port/api/v1/system/health" >"$uat_root/health-$port.json"; then
      return 0
    fi
    sleep 0.1
  done
  echo "MCG did not become healthy on port $port." >&2
  return 1
}

run_cli() {
  local data_root="$1"
  local port="$2"
  local executable="$3"
  shift 3
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$data_root" \
    "$executable" --host "http://127.0.0.1:$port" "$@"
}

dpkg -i "$deb" >"$uat_root/deb-install.log"
package_installed=true
for file in /usr/bin/mcg-desktop /usr/bin/mcg-service /usr/bin/mcg /usr/bin/mcg-shadow-runner \
  /usr/share/applications/MCG.desktop; do
  [[ -f "$file" ]] || { echo "Installed payload is missing $file." >&2; exit 1; }
done
find /usr/share/icons/hicolor -type f -name 'mcg-desktop.png' -print -quit | grep -q .
find /usr/lib/MCG -type f -name THIRD_PARTY_NOTICES.md -print -quit | grep -q .
find /usr/lib/MCG -type f -name Apache-2.0.txt -print -quit | grep -q .

sentinel="$installed_data/linux-package-uat-retained.txt"
runuser -u "$uat_user" -- sh -c 'printf retain-on-uninstall > "$1"' sh "$sentinel"
dpkg -i "$deb" >"$uat_root/deb-repair.log"
[[ -f "$sentinel" ]] || { echo "Repair installation removed external MCG data." >&2; exit 1; }

installed_port=17778
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" \
  MCG_DATA_DIR="$installed_data" MCG_BIND_ADDRESS="127.0.0.1:$installed_port" \
  /usr/bin/mcg-service >"$uat_root/installed-service.log" 2>&1 &
installed_service_pid=$!
background_pids+=("$installed_service_pid")
wait_for_health "$installed_port"
run_cli "$installed_data" "$installed_port" /usr/bin/mcg status --json >"$uat_root/status.json"
run_cli "$installed_data" "$installed_port" /usr/bin/mcg doctor --json >"$uat_root/doctor.json"
run_cli "$installed_data" "$installed_port" /usr/bin/mcg backup create --json >"$uat_root/backup.json"

python3 - "http://127.0.0.1:$installed_port/mcp" "$installed_data/local-auth-token" "$uat_root/mcp.json" <<'PY'
import json
import pathlib
import sys
import urllib.request

endpoint, token_path, output_path = sys.argv[1:]
token = pathlib.Path(token_path).read_text(encoding="utf-8").strip()

def call(payload):
    payload.setdefault("params", {})["_meta"] = {
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": {
            "name": "mcg-linux-package-uat",
            "version": "1.0.0",
        },
        "io.modelcontextprotocol/clientCapabilities": {},
    }
    request = urllib.request.Request(
        endpoint,
        data=json.dumps(payload).encode(),
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
            "MCP-Protocol-Version": "2026-07-28",
            "Mcp-Method": payload["method"],
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=10) as response:
        return json.load(response)

discovered = call({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "server/discover",
    "params": {},
})
assert discovered["result"]["supportedVersions"] == ["2026-07-28"]
assert discovered["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"] == "metacognitive-governor"
tools = call({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
names = [tool["name"] for tool in tools["result"]["tools"]]
assert "mcg_run_task" in names and "mcg_system_status" in names
pathlib.Path(output_path).write_text(
    json.dumps({"supportedVersion": "2026-07-28", "toolCount": len(names)}),
    encoding="utf-8",
)
PY

stop_tree "$installed_service_pid"
background_pids=()
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" \
  MCG_DATA_DIR="$installed_data" MCG_BIND_ADDRESS="127.0.0.1:$installed_port" \
  /usr/bin/mcg-service >"$uat_root/restarted-service.log" 2>&1 &
restarted_service_pid=$!
background_pids+=("$restarted_service_pid")
wait_for_health "$installed_port"
run_cli "$installed_data" "$installed_port" /usr/bin/mcg status --json >"$uat_root/restarted-status.json"
stop_tree "$restarted_service_pid"
background_pids=()

display=:97
runuser -u "$uat_user" -- Xvfb "$display" -screen 0 1280x800x24 -nolisten tcp >"$uat_root/xvfb.log" 2>&1 &
xvfb_pid=$!
background_pids+=("$xvfb_pid")
sleep 0.5

launch_and_capture() {
  local executable="$1"
  local data_root="$2"
  local port="$3"
  local label="$4"
  if ss -ltn "sport = :$port" | grep -q LISTEN; then
    echo "$label requires unused loopback port $port." >&2
    exit 1
  fi
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" DISPLAY="$display" \
    MCG_DATA_DIR="$data_root" MCG_BIND_ADDRESS="127.0.0.1:$port" \
    "$executable" >"$uat_root/$label-desktop.log" 2>&1 &
  local desktop_pid=$!
  background_pids+=("$desktop_pid")
  wait_for_health "$port"
  local window_id=""
  local attempt
  for attempt in $(seq 1 100); do
    window_id="$(runuser -u "$uat_user" -- env DISPLAY="$display" xdotool search --name '^MCG$' 2>/dev/null | head -n 1 || true)"
    [[ -n "$window_id" ]] && break
    sleep 0.1
  done
  [[ -n "$window_id" ]] || { echo "$label did not expose an MCG desktop window." >&2; exit 1; }
  runuser -u "$uat_user" -- env DISPLAY="$display" import -window root "$uat_root/$label.png"
  [[ -s "$uat_root/$label.png" ]] || { echo "$label screenshot is empty." >&2; exit 1; }
  local service_pid
  service_pid="$(ss -ltnp "sport = :$port" | sed -n 's/.*pid=\([0-9]\+\).*/\1/p' | head -n 1)"
  [[ "$service_pid" =~ ^[0-9]+$ ]] || { echo "$label service PID was not observable." >&2; exit 1; }
  [[ "$(basename "$(readlink -f "/proc/$service_pid/exe")")" == "mcg-service" ]] || {
    echo "$label loopback listener is not mcg-service." >&2
    exit 1
  }
  tr '\0' '\n' <"/proc/$service_pid/environ" | grep -Fxq "MCG_DATA_DIR=$data_root" || {
    echo "$label service is not bound to its isolated data directory." >&2
    exit 1
  }
  stop_tree "$desktop_pid"
  stop_tree "$service_pid"
  background_pids=("$xvfb_pid")
}

launch_and_capture /usr/bin/mcg-desktop "$uat_root/installed-desktop-data" 7777 installed

appimage_root="$uat_root/appimage"
mkdir "$appimage_root"
chown "$uat_user":"$(id -gn "$uat_user")" "$appimage_root"
runuser -u "$uat_user" -- sh -c 'cd "$1" && "$2" --appimage-extract >/dev/null' sh "$appimage_root" "$appimage"
for file in AppRun usr/bin/mcg-desktop usr/bin/mcg-service usr/bin/mcg usr/bin/mcg-shadow-runner \
  usr/share/applications/MCG.desktop; do
  [[ -f "$appimage_root/squashfs-root/$file" ]] || { echo "AppImage is missing $file." >&2; exit 1; }
done
find "$appimage_root/squashfs-root" -type f -name THIRD_PARTY_NOTICES.md -print -quit | grep -q .
find "$appimage_root/squashfs-root" -type f -name Apache-2.0.txt -print -quit | grep -q .
launch_and_capture "$appimage_root/squashfs-root/AppRun" "$appimage_data" 7777 appimage

dpkg -r "$package_name" >"$uat_root/deb-uninstall.log"
package_installed=false
[[ -f "$sentinel" ]] || { echo "Uninstall removed external MCG data." >&2; exit 1; }
[[ ! -e /usr/bin/mcg-desktop ]] || { echo "Uninstall retained the application executable." >&2; exit 1; }

dpkg -i "$deb" >"$uat_root/deb-reinstall.log"
package_installed=true
[[ -x /usr/bin/mcg-desktop ]] || { echo "Reinstall did not restore the application executable." >&2; exit 1; }
[[ -f "$sentinel" ]] || { echo "Reinstall removed external MCG data." >&2; exit 1; }
dpkg -r "$package_name" >"$uat_root/deb-final-uninstall.log"
package_installed=false
[[ -f "$sentinel" ]] || { echo "Final uninstall removed external MCG data." >&2; exit 1; }

cat <<EOF
Linux package UAT passed.
UAT_ROOT=$uat_root
DEB_PACKAGE=$package_name
INSTALLED_HEALTH_PORT=$installed_port
MCP_REPORT=$uat_root/mcp.json
INSTALLED_SCREENSHOT=$uat_root/installed.png
APPIMAGE_SCREENSHOT=$uat_root/appimage.png
DATA_RETAINED=true
REPAIR_PRESERVED_DATA=true
REINSTALL_PRESERVED_DATA=true
APPLICATION_REMOVED=true
EOF
