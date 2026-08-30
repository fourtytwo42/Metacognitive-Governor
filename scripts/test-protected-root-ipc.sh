#!/usr/bin/env bash
set -euo pipefail

root_executable="${1:-target/debug/mcg-protected-rootd}"
cli_executable="${2:-target/debug/mcg}"
port="${3:-17841}"
repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$root_executable" = /* ]]; then
  root_executable_path="$(realpath "$root_executable")"
else
  root_executable_path="$(realpath "$repository_root/$root_executable")"
fi
if [[ "$cli_executable" = /* ]]; then
  cli_executable_path="$(realpath "$cli_executable")"
else
  cli_executable_path="$(realpath "$repository_root/$cli_executable")"
fi
evidence_root="$repository_root/target/stage2-root-e2e"
mkdir -p "$evidence_root"
case_root="$(mktemp -d "$evidence_root/unix.XXXXXXXX")"
stdout="$case_root/root.stdout.log"
stderr="$case_root/root.stderr.log"
root_pid=""

cleanup() {
  if [[ -n "$root_pid" ]] && kill -0 "$root_pid" 2>/dev/null; then
    kill "$root_pid" 2>/dev/null || true
    wait "$root_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

MCG_DATA_DIR="$case_root" MCG_BIND_ADDRESS="127.0.0.1:$port" \
  "$root_executable_path" >"$stdout" 2>"$stderr" &
root_pid=$!

deadline=$((SECONDS + 30))
session_path="$case_root/root-ipc-session.json"
while [[ ! -f "$session_path" ]]; do
  if (( SECONDS > deadline )); then
    echo "Protected Root IPC session did not become ready." >&2
    exit 1
  fi
  sleep 0.1
done

export MCG_DATA_DIR="$case_root"
status_json="$($cli_executable_path runtime status --json)"
python3 -c 'import json,sys; s=json.load(sys.stdin); assert s["service"] == "protected-root"; assert s["storage_schema_version"] == 51; assert s["protocol_version"] == 1' <<<"$status_json"

inspect="$($cli_executable_path runtime inspect)"
grep -Fq "writer queue capacity: 64" <<<"$inspect"

health_json="$(curl --fail --silent --show-error "http://127.0.0.1:$port/api/v1/system/health")"
python3 -c 'import json,sys; s=json.load(sys.stdin); assert s["service"] == "mcg-daemon"; assert s["status"] == "ok"' <<<"$health_json"

legacy_status_json="$($cli_executable_path --host "http://127.0.0.1:$port" status --json)"
python3 -c 'import json,sys; s=json.load(sys.stdin); assert s["service"] == "mcg-daemon"; assert s["state"] == "ready"' <<<"$legacy_status_json"

stop_json="$($cli_executable_path --host "http://127.0.0.1:$port" stop --json)"
python3 -c 'import json,sys; s=json.load(sys.stdin); assert s["accepted"] is True; assert s["service"] == "mcg-daemon"' <<<"$stop_json"

deadline=$((SECONDS + 15))
while kill -0 "$root_pid" 2>/dev/null; do
  if (( SECONDS > deadline )); then
    echo "Protected Root did not stop cleanly." >&2
    exit 1
  fi
  sleep 0.1
done
wait "$root_pid"
root_pid=""

python3 - "$case_root" "$status_json" "$legacy_status_json" <<'PY'
import json
import sys

print(json.dumps({
    "schema_version": 1,
    "status": json.loads(sys.argv[2]),
    "legacy_status": json.loads(sys.argv[3]),
    "root_exit_code": 0,
    "evidence_directory": sys.argv[1],
}, indent=2))
PY
