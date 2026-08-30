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
codex_executable="${MCG_CODEX_APP_SERVER_EXECUTABLE:-$(command -v codex || true)}"

[[ -f "$deb" ]] || { echo "Missing deb package: $deb" >&2; exit 1; }
[[ -f "$appimage" ]] || { echo "Missing AppImage package: $appimage" >&2; exit 1; }
deb="$(realpath -- "$deb")"
appimage="$(realpath -- "$appimage")"
[[ "$uat_user" != "root" ]] || { echo "MCG_UAT_USER must name a non-root desktop user." >&2; exit 1; }
id "$uat_user" >/dev/null
[[ -n "$codex_executable" && -x "$codex_executable" ]] || {
  echo "Linux package UAT requires the pinned Codex executable through MCG_CODEX_APP_SERVER_EXECUTABLE or PATH." >&2
  exit 1
}
codex_executable="$(realpath -- "$codex_executable")"
[[ "$($codex_executable --version)" == "codex-cli 0.150.1" ]] || {
  echo "Linux package UAT requires codex-cli 0.150.1." >&2
  exit 1
}
for command in apparmor_parser apparmor_status curl dbus-daemon dpkg dpkg-deb find identify import npm pgrep python3 runuser script ss stat xdotool Xvfb; do
  command -v "$command" >/dev/null || { echo "Missing UAT dependency: $command" >&2; exit 1; }
done
python3 -c 'import pyatspi' >/dev/null 2>&1 || {
  echo "Missing UAT dependency: python3-pyatspi" >&2
  exit 1
}

package_name="$(dpkg-deb -f "$deb" Package)"
if dpkg-query -W -f='${Status}' "$package_name" 2>/dev/null | grep -q 'install ok installed'; then
  echo "Linux package UAT refuses to replace an existing $package_name installation." >&2
  exit 1
fi

uat_root="$(mktemp -d /tmp/mcg-linux-package-uat-XXXXXX)"
chown "$uat_user":"$(id -gn "$uat_user")" "$uat_root"
uat_home="$uat_root/home"
uat_runtime="$uat_root/runtime"
installed_data="$uat_root/installed-data"
appimage_data="$uat_root/appimage-data"
mkdir -p "$uat_home" "$uat_runtime" "$installed_data" "$appimage_data"
chown -R "$uat_user":"$(id -gn "$uat_user")" "$uat_home" "$uat_runtime" "$installed_data" "$appimage_data"
chmod 700 "$uat_runtime"

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

create_encrypted_backup_fixture() {
  local data_root="$1"
  local port="$2"
  local backup_id="$3"
  local bundle="$4"
  local report="$5"
  python3 - "$data_root/local-auth-token" "$port" "$backup_id" "$bundle" "$report" <<'PY'
import json
import hashlib
import pathlib
import sys
import urllib.request

token_path, port, backup_id, bundle_path, report_path = sys.argv[1:]
token = pathlib.Path(token_path).read_text(encoding="utf-8").strip()
passphrase = "package fixture passphrase 2026 encrypted backup"
request = urllib.request.Request(
    f"http://127.0.0.1:{port}/api/v1/backups/{backup_id}/encrypted",
    data=json.dumps({"passphrase": passphrase}).encode(),
    headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
    method="POST",
)
with urllib.request.urlopen(request, timeout=30) as response:
    result = json.load(response)
manifest = result["manifest"]
metadata = manifest["secret_encryption"]
assert result["backup_id"] == backup_id
assert manifest["schema_version"] == 2
assert manifest["secrets_included"] is True
assert metadata["schema_version"] == 1
assert metadata["entry_path"] == "secrets/credentials.v1.enc"
assert metadata["algorithm"] == "xchacha20-poly1305"
assert metadata["kdf"] == "argon2id"
assert metadata["argon2_version"] == 19
assert metadata["memory_kib"] == 65536
assert metadata["iterations"] == 3
assert metadata["parallelism"] == 1
assert manifest["entries"][-1]["path"] == "secrets/credentials.v1.enc"
bundle = pathlib.Path(bundle_path).read_bytes()
assert len(bundle) == result["size_bytes"]
assert hashlib.sha256(bundle).hexdigest() == result["sha256"]
assert passphrase.encode() not in bundle
assert token.encode() not in bundle
pathlib.Path(report_path).write_text(json.dumps(result), encoding="utf-8")
PY
}

restore_encrypted_backup_fixture() {
  local data_root="$1"
  local bundle="$2"
  local report="$3"
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$data_root" \
    python3 - /usr/bin/mcg "$bundle" "$data_root/local-auth-token" "$report" <<'PY'
import errno
import hashlib
import json
import os
import pathlib
import pty
import select
import signal
import sys
import time

executable, bundle, token_path, report_path = sys.argv[1:]
correct = "package fixture passphrase 2026 encrypted backup"
wrong = "package fixture passphrase that must be rejected"
token_before = pathlib.Path(token_path).read_bytes()

def restore(passphrase, expected_success):
    pid, terminal = pty.fork()
    if pid == 0:
        os.execv(executable, [
            executable, "backup", "restore", bundle, "--confirm", "--restore-secrets"
        ])
    output = bytearray()
    prompt = b"Encrypted backup passphrase:"
    prompt_deadline = time.monotonic() + 30
    while prompt not in output:
        if time.monotonic() >= prompt_deadline:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
            raise AssertionError(f"restore prompt timed out: {output.decode(errors='replace')}")
        readable, _, _ = select.select([terminal], [], [], 0.5)
        if not readable:
            exited, status = os.waitpid(pid, os.WNOHANG)
            if exited:
                raise AssertionError(
                    f"restore exited before its prompt with {os.waitstatus_to_exitcode(status)}: "
                    f"{output.decode(errors='replace')}"
                )
            continue
        try:
            output.extend(os.read(terminal, 4096))
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            _, status = os.waitpid(pid, 0)
            raise AssertionError(
                f"restore closed before its prompt with {os.waitstatus_to_exitcode(status)}: "
                f"{output.decode(errors='replace')}"
            ) from error
    os.write(terminal, passphrase.encode() + b"\n")
    completion_deadline = time.monotonic() + 120
    status = None
    while True:
        if time.monotonic() >= completion_deadline:
            os.kill(pid, signal.SIGKILL)
            _, status = os.waitpid(pid, 0)
            raise AssertionError(f"restore timed out: {output.decode(errors='replace')}")
        readable, _, _ = select.select([terminal], [], [], 0.5)
        if not readable:
            exited, status = os.waitpid(pid, os.WNOHANG)
            if exited:
                break
            continue
        try:
            chunk = os.read(terminal, 4096)
        except OSError as error:
            if error.errno == errno.EIO:
                _, status = os.waitpid(pid, 0)
                break
            raise
        if not chunk:
            _, status = os.waitpid(pid, 0)
            break
        output.extend(chunk)
    assert status is not None
    code = os.waitstatus_to_exitcode(status)
    text = output.decode(errors="replace")
    assert passphrase not in text
    assert (code == 0) is expected_success, (code, text)
    return code

wrong_code = restore(wrong, False)
correct_code = restore(correct, True)
token_after = pathlib.Path(token_path).read_bytes()
assert token_after == token_before
rollback_files = sorted(pathlib.Path(token_path).parent.joinpath("backups").glob("pre-restore-*.sqlite3"))
assert rollback_files
pathlib.Path(report_path).write_text(json.dumps({
    "wrong_passphrase_exit": wrong_code,
    "restore_exit": correct_code,
    "token_sha256": hashlib.sha256(token_after).hexdigest(),
    "rollback_database_count": len(rollback_files),
}), encoding="utf-8")
PY
}

test_consumer_cli() {
  local executable="$1"
  local data_root="$2"
  local port="$3"
  local label="$4"
  local project="$uat_root/$label-consumer-project"
  mkdir -p "$project"
  chown -R "$uat_user":"$(id -gn "$uat_user")" "$project"
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$data_root" \
    python3 - "$executable" "$data_root" "$port" "$uat_home" "$project" "$uat_root/$label-consumer-cli.json" <<'PY'
import json
import os
import pathlib
import shlex
import subprocess
import sys

executable, data_root, port, home, project, report_path = sys.argv[1:]
project_path = pathlib.Path(project)
project_path.joinpath("package.json").write_text(
    '{"scripts":{"test":"node --test"}}\n', encoding="utf-8"
)
env = os.environ.copy()
env.update({
    "HOME": home,
    "XDG_DATA_HOME": str(pathlib.Path(home) / ".local" / "share"),
    "MCG_DATA_DIR": data_root,
})
base = [
    executable,
    "--host", f"http://127.0.0.1:{port}",
    "--token-file", str(pathlib.Path(data_root) / "local-auth-token"),
    "-C", project,
]

def invoke(name, arguments=(), stdin=b""):
    result = subprocess.run(
        [*base, *arguments],
        input=stdin,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        timeout=30,
        check=False,
    )
    pathlib.Path(report_path).with_name(f"{name}.stdout").write_bytes(result.stdout)
    pathlib.Path(report_path).with_name(f"{name}.stderr").write_bytes(result.stderr)
    return result

baseline_tasks = invoke("consumer-baseline-tasks", ["task", "list", "--json"])
baseline_projects = invoke("consumer-baseline-projects", ["project", "list", "--json"])
assert baseline_tasks.returncode == 0 and baseline_projects.returncode == 0
baseline_task_count = len(json.loads(baseline_tasks.stdout)["tasks"])
baseline_project_count = len(json.loads(baseline_projects.stdout)["projects"])

bare = invoke(
    "consumer-bare",
    ["review this diff"],
    b"diff --git a/file.txt b/file.txt\n+private fixture content\n",
)
assert bare.returncode != 0
assert b"Status: blocked" in bare.stdout and b"requires attention" in bare.stderr
assert b"private fixture content" not in bare.stdout

structured = invoke("consumer-json", ["run", "--json", "fix", "the", "tests"])
assert structured.returncode != 0
structured_value = json.loads(structured.stdout)
assert structured_value["task"]["goal"] == "fix the tests"
assert structured_value["task"]["state"] == "blocked"

stream = invoke(
    "consumer-stream", ["run", "--stream-json", "inspect", "the", "project"]
)
assert stream.returncode != 0
records = [json.loads(line) for line in stream.stdout.splitlines() if line]
assert records[0]["type"] == "task" and records[-1]["type"] == "result"
assert any(record["type"] == "event" for record in records)

piped = invoke("consumer-piped", stdin=b"summarize this project\n")
assert piped.returncode != 0 and b"Status: blocked" in piped.stdout

oversized = invoke(
    "consumer-oversized", ["review oversized input"], b"x" * 1_048_577
)
assert oversized.returncode != 0 and b"1048576-byte limit" in oversized.stderr

interactive_command = [
    "script", "--quiet", "--return", "--command", shlex.join(base), "/dev/null"
]
interactive = subprocess.run(
    interactive_command,
    input=b"inspect through the interactive prompt\n:quit\n",
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    env=env,
    timeout=30,
    check=False,
)
pathlib.Path(report_path).with_name("consumer-interactive.stdout").write_bytes(
    interactive.stdout
)
pathlib.Path(report_path).with_name("consumer-interactive.stderr").write_bytes(
    interactive.stderr
)
assert interactive.returncode == 0
assert b"MCG interactive mode" in interactive.stdout
assert b"Status: blocked" in interactive.stdout and b"mcg>" in interactive.stdout

tasks = invoke("consumer-tasks", ["task", "list", "--json"])
projects = invoke("consumer-projects", ["project", "list", "--json"])
task_values = json.loads(tasks.stdout)["tasks"]
project_values = json.loads(projects.stdout)["projects"]
assert len(task_values) - baseline_task_count == 5
assert len(project_values) - baseline_project_count == 1

token = pathlib.Path(data_root, "local-auth-token").read_bytes().strip()
for result in [bare, structured, stream, piped, oversized, interactive, tasks, projects]:
    assert token not in result.stdout and token not in result.stderr

pathlib.Path(report_path).write_text(
    json.dumps({
        "schema_version": 1,
        "bare_blocked": True,
        "json_blocked": True,
        "stream_ndjson": True,
        "piped_context_private": True,
        "oversized_rejected": True,
        "interactive_tty": True,
        "task_count": len(task_values),
        "project_count": len(project_values),
        "created_task_count": len(task_values) - baseline_task_count,
        "created_project_count": len(project_values) - baseline_project_count,
    }),
    encoding="utf-8",
)
PY
}

test_plugin_runner() {
  local executable="$1"
  local label="$2"
  python3 - "$executable" "$uat_root/$label-plugin-runner.json" <<'PY'
import base64
import hashlib
import json
import pathlib
import subprocess
import sys

executable, report_path = sys.argv[1:]
module = b'''(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory 1)
  (export "memory" (memory 0))
  (data (i32.const 8) "{\\22package\\22:true}")
  (func (export "_start")
    (i32.store (i32.const 0) (i32.const 8))
    (i32.store (i32.const 4) (i32.const 16))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 24)))))'''
request = {
    "schema_version": 1,
    "manifest": {
        "schema_version": 1,
        "id": "uat.package",
        "display_name": "Package UAT",
        "version": "1.0.0",
        "class": "sandboxed_wasi",
        "requested_capabilities": [],
    },
    "module_sha256": hashlib.sha256(module).hexdigest(),
    "module_base64": base64.b64encode(module).decode("ascii"),
    "input": {},
}

def invoke(value):
    return subprocess.run(
        [executable],
        input=json.dumps(value).encode(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=15,
        check=False,
    )

valid = invoke(request)
assert valid.returncode == 0 and valid.stderr == b""
valid_response = json.loads(valid.stdout)
assert valid_response["state"] == "succeeded"
assert valid_response["result"]["output"] == {"package": True}
request["module_sha256"] = "0" * 64
tampered = invoke(request)
assert tampered.returncode != 0 and tampered.stderr == b""
tampered_response = json.loads(tampered.stdout)
assert tampered_response["error_code"] == "module_digest_mismatch"
pathlib.Path(report_path).write_text(
    json.dumps({"valid": valid_response["state"], "tampered": tampered_response["error_code"]}),
    encoding="utf-8",
)
PY
}

dpkg -i "$deb" >"$uat_root/deb-install.log"
package_installed=true
for file in /usr/bin/mcg-desktop /usr/bin/mcg-service /usr/bin/mcg /usr/bin/mcg-plugin-runner /usr/bin/mcg-shadow-runner /usr/bin/mcg-updater \
  /usr/lib/MCG/mcg-bwrap /etc/apparmor.d/metacognitive-governor-bwrap; do
  [[ -f "$file" ]] || { echo "Installed payload is missing $file." >&2; exit 1; }
done
[[ -f /usr/share/applications/MetaCog.desktop || -f /usr/share/applications/MCG.desktop ]] || {
  echo "Installed payload is missing its desktop entry." >&2
  exit 1
}
find /usr/share/icons/hicolor -type f -name 'mcg-desktop.png' -print -quit | grep -q .
find /usr/lib/MCG -type f -name THIRD_PARTY_NOTICES.md -print -quit | grep -q .
find /usr/lib/MCG -type f -name Apache-2.0.txt -print -quit | grep -q .
find /usr/lib/MCG -type f -name Wasmtime-Apache-2.0-WITH-LLVM-exception.txt -print -quit | grep -q .
find /usr/lib/MCG -type f -name Bubblewrap-GPL-2.0-or-later.txt -print -quit | grep -q .
find /usr/lib/MCG -type f -name bubblewrap-v0.11.2.tar.gz -print -quit | grep -q .
[[ "$(stat -c '%U:%G:%a' /usr/lib/MCG/mcg-bwrap)" == "root:root:755" ]] || {
  echo "Installed Bubblewrap is not a protected root-owned executable." >&2
  exit 1
}
[[ "$(/usr/lib/MCG/mcg-bwrap --version)" == "bubblewrap 0.11.2" ]]
apparmor_status | grep -Fxq '   metacognitive-governor-unpriv-bwrap'
/usr/lib/MCG/mcg-bwrap --unshare-all --unshare-user --disable-userns \
  --die-with-parent --ro-bind / / -- /usr/bin/true
test_plugin_runner /usr/bin/mcg-plugin-runner installed

sentinel="$installed_data/linux-package-uat-retained.txt"
runuser -u "$uat_user" -- sh -c 'printf retain-on-uninstall > "$1"' sh "$sentinel"
dpkg -i "$deb" >"$uat_root/deb-repair.log"
[[ -f "$sentinel" ]] || { echo "Repair installation removed external MCG data." >&2; exit 1; }

installed_port=17778
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" \
  MCG_DATA_DIR="$installed_data" MCG_BIND_ADDRESS="127.0.0.1:$installed_port" \
  MCG_CODEX_APP_SERVER_EXECUTABLE="$codex_executable" \
  /usr/bin/mcg-service >"$uat_root/installed-service.log" 2>&1 &
installed_service_pid=$!
background_pids+=("$installed_service_pid")
wait_for_health "$installed_port"
run_cli "$installed_data" "$installed_port" /usr/bin/mcg status --json >"$uat_root/status.json"
run_cli "$installed_data" "$installed_port" /usr/bin/mcg doctor --json >"$uat_root/doctor.json"
runuser -u "$uat_user" -- python3 "$repository_root/scripts/test-installed-experiments.py" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --state "$uat_root/experiment-state.json" --output "$uat_root/experiments.json"
provider_success_project="$uat_root/provider-success-project"
mkdir -p "$provider_success_project"
chown -R "$uat_user":"$(id -gn "$uat_user")" "$provider_success_project"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-provider-success.py" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --project "$provider_success_project" --state "$uat_root/provider-success-state.json" \
  --output "$uat_root/provider-success.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-memory-transfer.py" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --workspace "$uat_root" --state "$uat_root/memory-transfer-state.json" \
  --output "$uat_root/memory-transfer.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  node "$repository_root/scripts/test-installed-organization-sharing.mjs" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --state "$uat_root/organization-sharing-state.json" \
  --output "$uat_root/organization-sharing.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-interface-success.py" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --workspace "$uat_root" \
  --state "$uat_root/interface-success-state.json" \
  --output "$uat_root/interface-success.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-reviewed-topology.py" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --workspace "$uat_root" --state "$uat_root/reviewed-topology-state.json" \
  --output "$uat_root/reviewed-topology.json"
real_project="$uat_root/real-project"
mkdir -p "$real_project"
chown -R "$uat_user":"$(id -gn "$uat_user")" "$real_project"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-real-project.py" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --project "$real_project" --state "$uat_root/real-project-state.json" \
  --output "$uat_root/real-project.json"
real_api_project="$uat_root/real-api-project"
mkdir -p "$real_api_project"
chown -R "$uat_user":"$(id -gn "$uat_user")" "$real_api_project"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-real-api-project.py" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --project "$real_api_project" --state "$uat_root/real-api-state.json" \
  --output "$uat_root/real-api-project.json"
real_fullstack_project="$uat_root/homebase"
mkdir -p "$real_fullstack_project"
chown -R "$uat_user":"$(id -gn "$uat_user")" "$real_fullstack_project"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-real-fullstack-project.py" \
  --mode exercise --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --project "$real_fullstack_project" --state "$uat_root/real-fullstack-state.json" \
  --output "$uat_root/real-fullstack-project.json"
run_cli "$installed_data" "$installed_port" /usr/bin/mcg backup create --json >"$uat_root/backup.json"
encrypted_backup_id="00000000-0000-4000-8000-00000000b002"
encrypted_bundle="$installed_data/backups/$encrypted_backup_id.mcgb"
create_encrypted_backup_fixture \
  "$installed_data" "$installed_port" "$encrypted_backup_id" "$encrypted_bundle" \
  "$uat_root/encrypted-backup.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  /usr/bin/mcg backup verify "$encrypted_bundle" --json >"$uat_root/encrypted-backup-verified.json"
test_consumer_cli /usr/bin/mcg "$installed_data" "$installed_port" installed

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
assert {
    "mcg_run_task",
    "mcg_system_status",
    "mcg_experiments_inspect",
    "mcg_experiments_mutate",
}.issubset(names)
pathlib.Path(output_path).write_text(
    json.dumps({"supportedVersion": "2026-07-28", "toolCount": len(names)}),
    encoding="utf-8",
)
PY

stop_tree "$installed_service_pid"
background_pids=()
grep -Fq "MCG daemon stopped cleanly" "$uat_root/installed-service.log" || {
  echo "Packaged daemon did not report a graceful SIGTERM shutdown." >&2
  exit 1
}
[[ ! -e "$installed_data/mcg.sqlite3-wal" && ! -e "$installed_data/mcg.sqlite3-shm" ]] || {
  echo "Packaged daemon retained SQLite sidecars after graceful shutdown." >&2
  exit 1
}
restore_encrypted_backup_fixture \
  "$installed_data" "$encrypted_bundle" "$uat_root/encrypted-restore.json"
if grep -Fq "package fixture passphrase" "$uat_root/installed-service.log"; then
  echo "Encrypted backup passphrase leaked into the packaged service log." >&2
  exit 1
fi
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" \
  MCG_DATA_DIR="$installed_data" MCG_BIND_ADDRESS="127.0.0.1:$installed_port" \
  MCG_CODEX_APP_SERVER_EXECUTABLE="$codex_executable" \
  /usr/bin/mcg-service >"$uat_root/restarted-service.log" 2>&1 &
restarted_service_pid=$!
background_pids+=("$restarted_service_pid")
wait_for_health "$installed_port"
run_cli "$installed_data" "$installed_port" /usr/bin/mcg status --json >"$uat_root/restarted-status.json"
runuser -u "$uat_user" -- python3 "$repository_root/scripts/test-installed-experiments.py" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --state "$uat_root/experiment-state.json" --output "$uat_root/experiments-restart.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-provider-success.py" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --project "$provider_success_project" --state "$uat_root/provider-success-state.json" \
  --output "$uat_root/provider-success-restart.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-memory-transfer.py" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --workspace "$uat_root" --state "$uat_root/memory-transfer-state.json" \
  --output "$uat_root/memory-transfer-restart.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  node "$repository_root/scripts/test-installed-organization-sharing.mjs" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --state "$uat_root/organization-sharing-state.json" \
  --output "$uat_root/organization-sharing-restart.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-interface-success.py" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --workspace "$uat_root" \
  --state "$uat_root/interface-success-state.json" \
  --output "$uat_root/interface-success-restart.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-reviewed-topology.py" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --workspace "$uat_root" --state "$uat_root/reviewed-topology-state.json" \
  --output "$uat_root/reviewed-topology-restart.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-real-project.py" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --project "$real_project" --state "$uat_root/real-project-state.json" \
  --output "$uat_root/real-project-restart.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-real-api-project.py" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --project "$real_api_project" --state "$uat_root/real-api-state.json" \
  --output "$uat_root/real-api-project-restart.json"
runuser -u "$uat_user" -- env \
  HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" MCG_DATA_DIR="$installed_data" \
  python3 "$repository_root/scripts/test-installed-real-fullstack-project.py" \
  --mode verify --host "http://127.0.0.1:$installed_port" \
  --token-file "$installed_data/local-auth-token" --cli /usr/bin/mcg \
  --project "$real_fullstack_project" --state "$uat_root/real-fullstack-state.json" \
  --output "$uat_root/real-fullstack-project-restart.json"
stop_tree "$restarted_service_pid"
background_pids=()

display=:97
mapfile -t a11y_dbus < <(runuser -u "$uat_user" -- env XDG_RUNTIME_DIR="$uat_runtime" \
  dbus-daemon --session --fork --print-address=1 --print-pid=1)
[[ "${#a11y_dbus[@]}" -eq 2 && "${a11y_dbus[1]}" =~ ^[0-9]+$ ]] || {
  echo "Could not start the private accessibility D-Bus session." >&2
  exit 1
}
a11y_dbus_address="${a11y_dbus[0]}"
a11y_dbus_pid="${a11y_dbus[1]}"
runuser -u "$uat_user" -- Xvfb "$display" -screen 0 1280x800x24 -nolisten tcp >"$uat_root/xvfb.log" 2>&1 &
xvfb_pid=$!
background_pids+=("$a11y_dbus_pid" "$xvfb_pid")
sleep 0.5

launch_and_capture() {
  local executable="$1"
  local data_root="$2"
  local port="$3"
  local label="$4"
  local attempt
  if ss -ltn "sport = :$port" | grep -q LISTEN; then
    echo "$label requires unused loopback port $port." >&2
    exit 1
  fi
  local native_topology_workspace="$uat_root/$label-native-topology-workspace"
  local service_executable="$(dirname "$executable")/mcg-service"
  if [[ ! -x "$service_executable" && -x "$(dirname "$executable")/usr/bin/mcg-service" ]]; then
    service_executable="$(dirname "$executable")/usr/bin/mcg-service"
  fi
  [[ -x "$service_executable" ]] || {
    echo "$label is missing the adjacent packaged mcg-service." >&2
    exit 1
  }
  mkdir "$native_topology_workspace"
  chown "$uat_user":"$(id -gn "$uat_user")" "$native_topology_workspace"
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" XDG_RUNTIME_DIR="$uat_runtime" \
    MCG_DATA_DIR="$data_root" MCG_BIND_ADDRESS="127.0.0.1:$port" \
    MCG_CODEX_APP_SERVER_EXECUTABLE="$codex_executable" \
    "$service_executable" >"$uat_root/$label-prepare-service.log" 2>&1 &
  local prepare_service_pid=$!
  background_pids+=("$prepare_service_pid")
  wait_for_health "$port"
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" \
    python3 "$repository_root/scripts/test-installed-native-reviewed-topology.py" \
      --mode prepare --host "http://127.0.0.1:$port" \
      --token-file "$data_root/local-auth-token" --workspace "$native_topology_workspace" \
      --state "$uat_root/$label-native-reviewed-topology-state.json" \
      --output "$uat_root/$label-native-reviewed-topology-prepare.json"
  stop_tree "$prepare_service_pid"
  background_pids=("$a11y_dbus_pid" "$xvfb_pid")
  local stopped=false
  for attempt in $(seq 1 100); do
    if ! ss -ltn "sport = :$port" | grep -q LISTEN; then
      stopped=true
      break
    fi
    sleep 0.1
  done
  [[ "$stopped" == true ]] || { echo "$label prepare service did not stop." >&2; exit 1; }
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" XDG_RUNTIME_DIR="$uat_runtime" DISPLAY="$display" \
    DBUS_SESSION_BUS_ADDRESS="$a11y_dbus_address" NO_AT_BRIDGE=0 GTK_MODULES=gail:atk-bridge \
    MCG_DATA_DIR="$data_root" MCG_BIND_ADDRESS="127.0.0.1:$port" \
    MCG_CODEX_APP_SERVER_EXECUTABLE="$codex_executable" \
    "$executable" >"$uat_root/$label-desktop.log" 2>&1 &
  local desktop_pid=$!
  background_pids+=("$desktop_pid")
  wait_for_health "$port"
  local window_id=""
  for attempt in $(seq 1 100); do
    window_id="$(runuser -u "$uat_user" -- env DISPLAY="$display" xdotool search --name '^(MCG|MetaCog)( v[0-9].*)?$' 2>/dev/null | head -n 1 || true)"
    [[ -n "$window_id" ]] && break
    sleep 0.1
  done
  [[ -n "$window_id" ]] || { echo "$label did not expose an MCG desktop window." >&2; exit 1; }
  sleep 2
  runuser -u "$uat_user" -- env DISPLAY="$display" \
    xdotool windowfocus --sync "$window_id"
  runuser -u "$uat_user" -- env DISPLAY="$display" import -window "$window_id" "$uat_root/$label.png"
  [[ -s "$uat_root/$label.png" ]] || { echo "$label screenshot is empty." >&2; exit 1; }
  local screenshot_colors
  screenshot_colors="$(identify -format '%k' "$uat_root/$label.png")"
  [[ "$screenshot_colors" =~ ^[0-9]+$ && "$screenshot_colors" -gt 16 ]] || {
    echo "$label screenshot is blank or has too little rendered content." >&2
    exit 1
  }
  local service_pid
  service_pid="$(ss -ltnp "sport = :$port" | sed -n 's/.*pid=\([0-9]\+\).*/\1/p' | head -n 1)"
  [[ "$service_pid" =~ ^[0-9]+$ ]] || { echo "$label service PID was not observable." >&2; exit 1; }
  [[ "$(basename "$(readlink -f "/proc/$service_pid/exe")")" == "mcg-service" ]] || {
    echo "$label loopback listener is not mcg-service." >&2
    exit 1
  }
  background_pids+=("$service_pid")
  tr '\0' '\n' <"/proc/$service_pid/environ" | grep -Fxq "MCG_DATA_DIR=$data_root" || {
    echo "$label service is not bound to its isolated data directory." >&2
    exit 1
  }
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" XDG_RUNTIME_DIR="$uat_runtime" DISPLAY="$display" \
    DBUS_SESSION_BUS_ADDRESS="$a11y_dbus_address" \
    python3 "$repository_root/scripts/test-installed-native-accessibility-linux.py" \
      --output "$uat_root/$label-native-accessibility.json" \
      --host "http://127.0.0.1:$port" \
      --token-file "$data_root/local-auth-token" \
      --reviewed-topology-marker "$native_topology_workspace/native-reviewed-topology-project/execution-count.txt"
  runuser -u "$uat_user" -- env \
    HOME="$uat_home" XDG_DATA_HOME="$uat_home/.local/share" \
    python3 "$repository_root/scripts/test-installed-native-reviewed-topology.py" \
      --mode verify --host "http://127.0.0.1:$port" \
      --token-file "$data_root/local-auth-token" --workspace "$native_topology_workspace" \
      --state "$uat_root/$label-native-reviewed-topology-state.json" \
      --output "$uat_root/$label-native-reviewed-topology-verify.json"
  runuser -u "$uat_user" -- env DISPLAY="$display" \
    import -window "$window_id" "$uat_root/$label-reviewed-topology.png"
  [[ -s "$uat_root/$label-reviewed-topology.png" ]] || {
    echo "$label reviewed-topology screenshot is empty." >&2
    exit 1
  }
  stop_tree "$desktop_pid"
  stop_tree "$service_pid"
  background_pids=("$a11y_dbus_pid" "$xvfb_pid")
}

launch_and_capture /usr/bin/mcg-desktop "$uat_root/installed-desktop-data" 7777 installed

appimage_root="$uat_root/appimage"
mkdir "$appimage_root"
chown "$uat_user":"$(id -gn "$uat_user")" "$appimage_root"
runuser -u "$uat_user" -- sh -c 'cd "$1" && "$2" --appimage-extract >/dev/null' sh "$appimage_root" "$appimage"
for file in AppRun usr/bin/mcg-desktop usr/bin/mcg-service usr/bin/mcg usr/bin/mcg-plugin-runner usr/bin/mcg-shadow-runner usr/bin/mcg-updater; do
  [[ -f "$appimage_root/squashfs-root/$file" ]] || { echo "AppImage is missing $file." >&2; exit 1; }
done
[[ -f "$appimage_root/squashfs-root/usr/share/applications/MetaCog.desktop" || \
   -f "$appimage_root/squashfs-root/usr/share/applications/MCG.desktop" ]] || {
  echo "AppImage is missing its desktop entry." >&2
  exit 1
}
find "$appimage_root/squashfs-root" -type f -name THIRD_PARTY_NOTICES.md -print -quit | grep -q .
find "$appimage_root/squashfs-root" -type f -name Apache-2.0.txt -print -quit | grep -q .
find "$appimage_root/squashfs-root" -type f -name Wasmtime-Apache-2.0-WITH-LLVM-exception.txt -print -quit | grep -q .
test_plugin_runner "$appimage_root/squashfs-root/usr/bin/mcg-plugin-runner" appimage
launch_and_capture "$appimage_root/squashfs-root/AppRun" "$appimage_data" 7777 appimage

dpkg -r "$package_name" >"$uat_root/deb-uninstall.log"
package_installed=false
[[ -f "$sentinel" ]] || { echo "Uninstall removed external MCG data." >&2; exit 1; }
[[ ! -e /usr/bin/mcg-desktop ]] || { echo "Uninstall retained the application executable." >&2; exit 1; }
[[ ! -e /usr/lib/MCG/mcg-bwrap && ! -e /etc/apparmor.d/metacognitive-governor-bwrap ]] || {
  echo "Uninstall retained the packaged Linux sandbox boundary." >&2
  exit 1
}
if apparmor_status | grep -Fq 'metacognitive-governor-unpriv-bwrap'; then
  echo "Uninstall retained the MCG Bubblewrap AppArmor profile." >&2
  exit 1
fi

dpkg -i "$deb" >"$uat_root/deb-reinstall.log"
package_installed=true
[[ -x /usr/bin/mcg-desktop ]] || { echo "Reinstall did not restore the application executable." >&2; exit 1; }
[[ -x /usr/lib/MCG/mcg-bwrap && -f /etc/apparmor.d/metacognitive-governor-bwrap ]] || {
  echo "Reinstall did not restore the protected Linux sandbox boundary." >&2
  exit 1
}
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
CONSUMER_CLI_REPORT=$uat_root/installed-consumer-cli.json
PROVIDER_SUCCESS_REPORT=$uat_root/provider-success.json
PROVIDER_SUCCESS_RESTART_REPORT=$uat_root/provider-success-restart.json
ORGANIZATION_SHARING_REPORT=$uat_root/organization-sharing.json
ORGANIZATION_SHARING_RESTART_REPORT=$uat_root/organization-sharing-restart.json
REVIEWED_TOPOLOGY_REPORT=$uat_root/reviewed-topology.json
REVIEWED_TOPOLOGY_RESTART_REPORT=$uat_root/reviewed-topology-restart.json
REAL_PROJECT_REPORT=$uat_root/real-project.json
REAL_PROJECT_RESTART_REPORT=$uat_root/real-project-restart.json
REAL_API_PROJECT_REPORT=$uat_root/real-api-project.json
REAL_API_PROJECT_RESTART_REPORT=$uat_root/real-api-project-restart.json
REAL_FULLSTACK_PROJECT_REPORT=$uat_root/real-fullstack-project.json
REAL_FULLSTACK_PROJECT_RESTART_REPORT=$uat_root/real-fullstack-project-restart.json
INSTALLED_SCREENSHOT=$uat_root/installed.png
APPIMAGE_SCREENSHOT=$uat_root/appimage.png
INSTALLED_NATIVE_ACCESSIBILITY=$uat_root/installed-native-accessibility.json
APPIMAGE_NATIVE_ACCESSIBILITY=$uat_root/appimage-native-accessibility.json
INSTALLED_NATIVE_REVIEWED_TOPOLOGY=$uat_root/installed-native-reviewed-topology-verify.json
APPIMAGE_NATIVE_REVIEWED_TOPOLOGY=$uat_root/appimage-native-reviewed-topology-verify.json
INSTALLED_REVIEWED_TOPOLOGY_SCREENSHOT=$uat_root/installed-reviewed-topology.png
APPIMAGE_REVIEWED_TOPOLOGY_SCREENSHOT=$uat_root/appimage-reviewed-topology.png
DATA_RETAINED=true
REPAIR_PRESERVED_DATA=true
REINSTALL_PRESERVED_DATA=true
APPLICATION_REMOVED=true
PLUGIN_RUNNERS_QUALIFIED=true
ENCRYPTED_BACKUP_RESTORE_REPORT=$uat_root/encrypted-restore.json
EOF
