#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS package UAT must run on macOS." >&2
  exit 1
fi

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
native_accessibility_verifier="$repository_root/scripts/test-installed-native-accessibility-macos.swift"
source_app="${1:-$repository_root/target/release/bundle/macos/MCG.app}"
external_capture_request="${MCG_MACOS_UAT_EXTERNAL_CAPTURE_REQUEST:-}"
external_capture_timeout_seconds="${MCG_MACOS_UAT_EXTERNAL_CAPTURE_TIMEOUT_SECONDS:-600}"
[[ "$external_capture_timeout_seconds" =~ ^[0-9]+$ \
  && "$external_capture_timeout_seconds" -ge 30 \
  && "$external_capture_timeout_seconds" -le 1800 ]] || {
  echo "MCG_MACOS_UAT_EXTERNAL_CAPTURE_TIMEOUT_SECONDS must be an integer from 30 through 1800." >&2
  exit 1
}
case "$(uname -m)" in
  x86_64) bundle_arch=x64 ;;
  arm64|aarch64) bundle_arch=aarch64 ;;
  *) echo "Unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
esac
dmg="${2:-$repository_root/target/release/bundle/dmg/MCG_0.1.0_${bundle_arch}.dmg}"

[[ -d "$source_app" ]] || { echo "Missing app bundle: $source_app" >&2; exit 1; }
[[ -f "$dmg" ]] || { echo "Missing disk image: $dmg" >&2; exit 1; }
[[ -f "$native_accessibility_verifier" ]] || { echo "Missing macOS native accessibility verifier: $native_accessibility_verifier" >&2; exit 1; }
for command in codesign curl ditto hdiutil lsof node npm open pgrep plutil python3 screencapture sips spctl xcrun; do
  command -v "$command" >/dev/null || { echo "Missing UAT dependency: $command" >&2; exit 1; }
done

uat_root="$(mktemp -d "${TMPDIR:-/tmp}/mcg-macos-package-uat-XXXXXX")"
uat_root="$(cd "$uat_root" && pwd -P)"
mount_point="$uat_root/dmg"
install_root="$uat_root/Applications"
uat_home="$uat_root/home"
service_data="$uat_root/service-data"
gui_data="$uat_root/gui-data"
installed_app="$install_root/MCG.app"
mkdir -p "$mount_point" "$install_root" "$uat_home" "$service_data" "$gui_data"

mounted=false
background_pids=()

stop_process() {
  local pid="${1:-}"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 0
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    for _ in {1..40}; do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    kill -9 "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
}

cleanup() {
  local pid
  for pid in "${background_pids[@]:-}"; do stop_process "$pid"; done
  if [[ "$mounted" == true ]]; then hdiutil detach "$mount_point" -quiet >/dev/null 2>&1 || true; fi
}
trap cleanup EXIT

remove_installed_app() {
  local parent
  parent="$(cd "$(dirname "$installed_app")" && pwd -P)"
  if [[ "$parent" != "$uat_root/Applications" || "$(basename "$installed_app")" != "MCG.app" || "$uat_root" == "/" ]]; then
    echo "Refusing to remove an app outside the exact UAT installation root." >&2
    exit 1
  fi
  rm -rf -- "$installed_app"
}

wait_for_health() {
  local port="$1"
  for _ in {1..120}; do
    if curl --fail --silent "http://127.0.0.1:$port/api/v1/system/health" >"$uat_root/health-$port.json"; then
      return 0
    fi
    sleep 0.1
  done
  echo "MCG did not become healthy on port $port." >&2
  return 1
}

verify_signature() {
  local app="$1"
  local label="$2"
  local report="$uat_root/$label-codesign.txt"
  if codesign --verify --deep --strict --verbose=4 "$app" 2>"$report"; then
    codesign --display --verbose=4 "$app" 2>>"$report"
    if grep -Fq "Authority=Developer ID Application:" "$report"; then
      printf '%s\n' developer-id
    else
      [[ "${MCG_MACOS_UAT_ALLOW_ADHOC:-}" == "1" ]] || {
        echo "The $label app is not Developer ID signed. Set MCG_MACOS_UAT_ALLOW_ADHOC=1 only for explicit owner-built pre-release UAT." >&2
        exit 1
      }
      printf '%s\n' ad-hoc
    fi
  else
    [[ "${MCG_MACOS_UAT_ALLOW_UNSIGNED:-}" == "1" ]] || {
      echo "The $label app has no valid code signature. Set MCG_MACOS_UAT_ALLOW_UNSIGNED=1 only for explicit owner-built pre-release UAT." >&2
      exit 1
    }
    printf '%s\n' unsigned
  fi
  if [[ "${MCG_MACOS_UAT_REQUIRE_NOTARIZED:-}" == "1" ]]; then
    spctl --assess --type execute --verbose=4 "$app" 2>"$uat_root/$label-spctl.txt"
    xcrun stapler validate "$app" >"$uat_root/$label-stapler.txt" 2>&1
  fi
}

verify_payload() {
  local app="$1"
  for file in mcg-desktop mcg-service mcg mcg-plugin-runner mcg-shadow-runner mcg-updater; do
    [[ -x "$app/Contents/MacOS/$file" ]] || { echo "App bundle is missing executable $file." >&2; exit 1; }
  done
  [[ "$(plutil -extract CFBundleIdentifier raw "$app/Contents/Info.plist")" == "dev.mcg.desktop" ]] || {
    echo "App bundle identifier is not dev.mcg.desktop." >&2
    exit 1
  }
  for resource in THIRD_PARTY_NOTICES.md Apache-2.0.txt Wasmtime-Apache-2.0-WITH-LLVM-exception.txt; do
    find "$app/Contents/Resources" -type f -name "$resource" -print -quit | grep -q . || {
      echo "App bundle is missing $resource." >&2
      exit 1
    }
  done
}

test_plugin_runner() {
  local executable="$1"
  local report="$2"
  node --input-type=module - "$executable" "$report" <<'NODE'
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";

const [executable, report] = process.argv.slice(2);
const moduleBytes = Buffer.from(`(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory 1)
  (export "memory" (memory 0))
  (data (i32.const 8) "{\\22package\\22:true}")
  (func (export "_start")
    (i32.store (i32.const 0) (i32.const 8))
    (i32.store (i32.const 4) (i32.const 16))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 24)))))`);
const request = {
  schema_version: 1,
  manifest: {
    schema_version: 1,
    id: "uat.package",
    display_name: "Package UAT",
    version: "1.0.0",
    class: "sandboxed_wasi",
    requested_capabilities: []
  },
  module_sha256: createHash("sha256").update(moduleBytes).digest("hex"),
  module_base64: moduleBytes.toString("base64"),
  input: {}
};
const invoke = (value) => spawnSync(executable, [], {
  input: JSON.stringify(value),
  encoding: "utf8",
  timeout: 15_000,
  maxBuffer: 1024 * 1024
});
const valid = invoke(request);
if (valid.status !== 0 || valid.stderr !== "") throw new Error("valid packaged plugin invocation failed");
const validResponse = JSON.parse(valid.stdout);
if (validResponse.state !== "succeeded" || validResponse.result.output.package !== true) throw new Error("packaged plugin result mismatch");
request.module_sha256 = "0".repeat(64);
const tampered = invoke(request);
if (tampered.status === 0 || tampered.stderr !== "") throw new Error("tampered packaged plugin did not fail closed");
const tamperedResponse = JSON.parse(tampered.stdout);
if (tamperedResponse.error_code !== "module_digest_mismatch") throw new Error("tampered packaged plugin error mismatch");
writeFileSync(report, JSON.stringify({valid: validResponse.state, tampered: tamperedResponse.error_code}));
NODE
}

hdiutil verify "$dmg" >"$uat_root/dmg-verify.txt"
hdiutil attach -readonly -nobrowse -mountpoint "$mount_point" "$dmg" >"$uat_root/dmg-attach.txt"
mounted=true
mounted_app="$(find "$mount_point" -maxdepth 2 -type d -name 'MCG.app' -print -quit)"
[[ -n "$mounted_app" ]] || { echo "Disk image does not contain MCG.app." >&2; exit 1; }

source_signature="$(verify_signature "$source_app" source)"
mounted_signature="$(verify_signature "$mounted_app" mounted)"
verify_payload "$mounted_app"
ditto "$mounted_app" "$installed_app"
installed_signature="$(verify_signature "$installed_app" installed)"
verify_payload "$installed_app"
test_plugin_runner "$installed_app/Contents/MacOS/mcg-plugin-runner" "$uat_root/plugin-runner.json"

sentinel="$service_data/macos-package-uat-retained.txt"
printf '%s' retain-on-uninstall >"$sentinel"
ditto "$mounted_app" "$installed_app"
[[ -f "$sentinel" ]] || { echo "Repair installation removed external MCG data." >&2; exit 1; }

service_port=17779
if lsof -nP -iTCP:"$service_port" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "Service UAT requires unused loopback port $service_port." >&2
  exit 1
fi
env HOME="$uat_home" MCG_DATA_DIR="$service_data" MCG_BIND_ADDRESS="127.0.0.1:$service_port" \
  "$installed_app/Contents/MacOS/mcg-service" >"$uat_root/service.log" 2>&1 &
service_pid=$!
background_pids+=("$service_pid")
wait_for_health "$service_port"
cli=("$installed_app/Contents/MacOS/mcg" --host "http://127.0.0.1:$service_port" --token-file "$service_data/local-auth-token")
env HOME="$uat_home" "${cli[@]}" status --json >"$uat_root/status.json"
env HOME="$uat_home" "${cli[@]}" doctor --json >"$uat_root/doctor.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-experiments.py" \
  --mode exercise --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" \
  --state "$uat_root/experiment-state.json" --output "$uat_root/experiments.json"
provider_success_project="$uat_root/provider-success-project"
mkdir -p "$provider_success_project"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-provider-success.py" \
  --mode exercise --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --project "$provider_success_project" \
  --state "$uat_root/provider-success-state.json" --output "$uat_root/provider-success.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-memory-transfer.py" \
  --mode exercise --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --workspace "$uat_root" \
  --state "$uat_root/memory-transfer-state.json" --output "$uat_root/memory-transfer.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-interface-success.py" \
  --mode exercise --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" --workspace "$uat_root" \
  --state "$uat_root/interface-success-state.json" \
  --output "$uat_root/interface-success.json"
real_project="$uat_root/real-project"
mkdir -p "$real_project"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-real-project.py" \
  --mode exercise --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --project "$real_project" \
  --state "$uat_root/real-project-state.json" --output "$uat_root/real-project.json"
real_api_project="$uat_root/real-api-project"
mkdir -p "$real_api_project"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-real-api-project.py" \
  --mode exercise --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --project "$real_api_project" \
  --state "$uat_root/real-api-state.json" --output "$uat_root/real-api-project.json"
real_fullstack_project="$uat_root/homebase"
mkdir -p "$real_fullstack_project"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-real-fullstack-project.py" \
  --mode exercise --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --project "$real_fullstack_project" \
  --state "$uat_root/real-fullstack-state.json" --output "$uat_root/real-fullstack-project.json"
env HOME="$uat_home" "${cli[@]}" backup create --json >"$uat_root/backup.json"

node --input-type=module - "http://127.0.0.1:$service_port/mcp" "$service_data/local-auth-token" "$uat_root/mcp.json" <<'NODE'
import { readFileSync, writeFileSync } from "node:fs";
const [endpoint, tokenPath, outputPath] = process.argv.slice(2);
const token = readFileSync(tokenPath, "utf8").trim();
const call = async (payload) => {
  payload.params ??= {};
  payload.params._meta = {
    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
    "io.modelcontextprotocol/clientInfo": {name: "mcg-macos-package-uat", version: "1.0.0"},
    "io.modelcontextprotocol/clientCapabilities": {}
  };
  const response = await fetch(endpoint, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      Accept: "application/json, text/event-stream",
      "MCP-Protocol-Version": "2026-07-28",
      "Mcp-Method": payload.method
    },
    body: JSON.stringify(payload)
  });
  if (!response.ok) throw new Error(`MCP HTTP ${response.status}`);
  return response.json();
};
const discovered = await call({jsonrpc: "2.0", id: 1, method: "server/discover", params: {}});
if (discovered.result.supportedVersions[0] !== "2026-07-28") throw new Error("MCP version mismatch");
const tools = await call({jsonrpc: "2.0", id: 2, method: "tools/list", params: {}});
const names = tools.result.tools.map((tool) => tool.name);
for (const name of ["mcg_run_task", "mcg_system_status", "mcg_experiments_inspect", "mcg_experiments_mutate"]) {
  if (!names.includes(name)) throw new Error(`MCP tool catalog missing ${name}`);
}
writeFileSync(outputPath, JSON.stringify({supportedVersion: "2026-07-28", toolCount: names.length}));
NODE

stop_process "$service_pid"
background_pids=()
env HOME="$uat_home" MCG_DATA_DIR="$service_data" MCG_BIND_ADDRESS="127.0.0.1:$service_port" \
  "$installed_app/Contents/MacOS/mcg-service" >"$uat_root/restarted-service.log" 2>&1 &
restarted_service_pid=$!
background_pids+=("$restarted_service_pid")
wait_for_health "$service_port"
env HOME="$uat_home" "${cli[@]}" status --json >"$uat_root/restarted-status.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-experiments.py" \
  --mode verify --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" \
  --state "$uat_root/experiment-state.json" --output "$uat_root/experiments-restart.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-provider-success.py" \
  --mode verify --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --project "$provider_success_project" \
  --state "$uat_root/provider-success-state.json" \
  --output "$uat_root/provider-success-restart.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-memory-transfer.py" \
  --mode verify --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --workspace "$uat_root" \
  --state "$uat_root/memory-transfer-state.json" \
  --output "$uat_root/memory-transfer-restart.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-interface-success.py" \
  --mode verify --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" --workspace "$uat_root" \
  --state "$uat_root/interface-success-state.json" \
  --output "$uat_root/interface-success-restart.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-real-project.py" \
  --mode verify --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --project "$real_project" \
  --state "$uat_root/real-project-state.json" \
  --output "$uat_root/real-project-restart.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-real-api-project.py" \
  --mode verify --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --project "$real_api_project" \
  --state "$uat_root/real-api-state.json" \
  --output "$uat_root/real-api-project-restart.json"
env HOME="$uat_home" python3 "$repository_root/scripts/test-installed-real-fullstack-project.py" \
  --mode verify --host "http://127.0.0.1:$service_port" \
  --token-file "$service_data/local-auth-token" \
  --cli "$installed_app/Contents/MacOS/mcg" --project "$real_fullstack_project" \
  --state "$uat_root/real-fullstack-state.json" \
  --output "$uat_root/real-fullstack-project-restart.json"
stop_process "$restarted_service_pid"
background_pids=()

gui_port=7777
if lsof -nP -iTCP:"$gui_port" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "Desktop UAT requires unused loopback port $gui_port." >&2
  exit 1
fi
env HOME="$uat_home" MCG_DATA_DIR="$gui_data" MCG_BIND_ADDRESS="127.0.0.1:$gui_port" \
  "$installed_app/Contents/MacOS/mcg-desktop" >"$uat_root/desktop.log" 2>&1 &
desktop_pid=$!
background_pids+=("$desktop_pid")
wait_for_health "$gui_port"
for _ in {1..100}; do
  kill -0 "$desktop_pid" 2>/dev/null || { echo "Packaged desktop exited before UAT capture." >&2; exit 1; }
  pgrep -f "mcg-desktop" >/dev/null && break
  sleep 0.1
done
desktop_service_pid="$(lsof -nP -tiTCP:"$gui_port" -sTCP:LISTEN | head -n 1)"
[[ "$desktop_service_pid" =~ ^[0-9]+$ ]] || { echo "Packaged desktop service PID was not observable." >&2; exit 1; }
[[ "$(ps -p "$desktop_service_pid" -o comm=)" == *mcg-service ]] || { echo "Desktop loopback listener is not mcg-service." >&2; exit 1; }
background_pids+=("$desktop_service_pid")
sleep 2
if [[ -n "$external_capture_request" ]]; then
  [[ "$external_capture_request" == /* ]] || { echo "External capture request path must be absolute." >&2; exit 1; }
  [[ ! -e "$external_capture_request" ]] || { echo "Refusing to replace external capture request: $external_capture_request" >&2; exit 1; }
  (set -o noclobber; printf '%s\n' "$uat_root/desktop.png" > "$external_capture_request")
  capture_attempts=$((external_capture_timeout_seconds * 10))
  for ((capture_attempt = 0; capture_attempt < capture_attempts; capture_attempt++)); do
    [[ -s "$uat_root/desktop.png" ]] && break
    sleep 0.1
  done
else
  screencapture -x "$uat_root/desktop.png"
fi
[[ -s "$uat_root/desktop.png" ]] || { echo "Packaged desktop screenshot is empty." >&2; exit 1; }
pixel_width="$(sips -g pixelWidth "$uat_root/desktop.png" | awk '/pixelWidth:/{print $2}')"
pixel_height="$(sips -g pixelHeight "$uat_root/desktop.png" | awk '/pixelHeight:/{print $2}')"
[[ "$pixel_width" =~ ^[0-9]+$ && "$pixel_width" -ge 760 && "$pixel_height" =~ ^[0-9]+$ && "$pixel_height" -ge 540 ]] || {
  echo "Packaged desktop screenshot dimensions are invalid." >&2
  exit 1
}
xcrun swift "$native_accessibility_verifier" "$desktop_pid" "$uat_root/desktop-native-accessibility.json"
stop_process "$desktop_pid"
stop_process "$desktop_service_pid"
background_pids=()

remove_installed_app
[[ -f "$sentinel" ]] || { echo "Uninstall removed external MCG data." >&2; exit 1; }
[[ ! -e "$installed_app" ]] || { echo "Uninstall retained the application bundle." >&2; exit 1; }
ditto "$mounted_app" "$installed_app"
verify_payload "$installed_app"
[[ -f "$sentinel" ]] || { echo "Reinstall removed external MCG data." >&2; exit 1; }
remove_installed_app
[[ -f "$sentinel" ]] || { echo "Final uninstall removed external MCG data." >&2; exit 1; }

hdiutil detach "$mount_point" -quiet
mounted=false

cat <<EOF
macOS package UAT passed.
UAT_ROOT=$uat_root
SOURCE_SIGNATURE=$source_signature
MOUNTED_SIGNATURE=$mounted_signature
INSTALLED_SIGNATURE=$installed_signature
SERVICE_HEALTH_PORT=$service_port
MCP_REPORT=$uat_root/mcp.json
NATIVE_ACCESSIBILITY_REPORT=$uat_root/desktop-native-accessibility.json
PROVIDER_SUCCESS_REPORT=$uat_root/provider-success.json
PROVIDER_SUCCESS_RESTART_REPORT=$uat_root/provider-success-restart.json
REAL_PROJECT_REPORT=$uat_root/real-project.json
REAL_PROJECT_RESTART_REPORT=$uat_root/real-project-restart.json
REAL_API_PROJECT_REPORT=$uat_root/real-api-project.json
REAL_API_PROJECT_RESTART_REPORT=$uat_root/real-api-project-restart.json
REAL_FULLSTACK_PROJECT_REPORT=$uat_root/real-fullstack-project.json
REAL_FULLSTACK_PROJECT_RESTART_REPORT=$uat_root/real-fullstack-project-restart.json
DESKTOP_SCREENSHOT=$uat_root/desktop.png
EXTERNAL_CAPTURE_REQUEST=${external_capture_request:-none}
DATA_RETAINED=true
REPAIR_PRESERVED_DATA=true
REINSTALL_PRESERVED_DATA=true
APPLICATION_REMOVED=true
PLUGIN_RUNNERS_QUALIFIED=true
NOTARIZATION_REQUIRED=${MCG_MACOS_UAT_REQUIRE_NOTARIZED:-0}
EOF
