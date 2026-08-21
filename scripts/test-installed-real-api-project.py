#!/usr/bin/env python3
"""Build and verify a representative local JSON API through an installed MCG package."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import socket
import subprocess
import threading
import time
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


SCHEMA_VERSION = 1
MODEL = "installed-real-api-project-model"

LEDGER_JS = """export function createLedger() {
  const entries = [];
  let nextId = 1;
  return {
    add(input) {
      if (!input || typeof input !== 'object' || Array.isArray(input)) {
        throw new TypeError('entry must be an object');
      }
      if (!Number.isInteger(input.amountCents) || input.amountCents <= 0) {
        throw new TypeError('amountCents must be a positive integer');
      }
      const category = typeof input.category === 'string' ? input.category.trim() : '';
      if (!category || category.length > 40) {
        throw new TypeError('category must contain 1 through 40 characters');
      }
      const note = input.note === undefined ? '' : input.note;
      if (typeof note !== 'string' || note.length > 120) {
        throw new TypeError('note must contain at most 120 characters');
      }
      const entry = Object.freeze({ id: nextId++, amountCents: input.amountCents, category, note });
      entries.push(entry);
      return { ...entry };
    },
    summary() {
      const byCategory = {};
      for (const entry of entries) {
        byCategory[entry.category] = (byCategory[entry.category] ?? 0) + entry.amountCents;
      }
      return {
        count: entries.length,
        totalCents: entries.reduce((total, entry) => total + entry.amountCents, 0),
        byCategory,
      };
    },
  };
}
"""

SERVER_JS = """import http from 'node:http';
import { createLedger } from './ledger.mjs';

const ledger = createLedger();
const MAX_BODY_BYTES = 32 * 1024;

function send(response, status, value) {
  const body = JSON.stringify(value);
  response.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store',
    'content-length': Buffer.byteLength(body),
  });
  response.end(body);
}

async function readJson(request) {
  const chunks = [];
  let length = 0;
  for await (const chunk of request) {
    length += chunk.length;
    if (length > MAX_BODY_BYTES) {
      const error = new Error('request body is too large');
      error.status = 413;
      throw error;
    }
    chunks.push(chunk);
  }
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch {
    const error = new Error('request body must be valid JSON');
    error.status = 400;
    throw error;
  }
}

export const server = http.createServer(async (request, response) => {
  try {
    if (request.method === 'GET' && request.url === '/health') {
      send(response, 200, { status: 'ok' });
      return;
    }
    if (request.method === 'POST' && request.url === '/entries') {
      const entry = ledger.add(await readJson(request));
      send(response, 201, { entry });
      return;
    }
    if (request.method === 'GET' && request.url === '/summary') {
      send(response, 200, ledger.summary());
      return;
    }
    send(response, 404, { error: 'not_found' });
  } catch (error) {
    const status = Number.isInteger(error.status) ? error.status : error instanceof TypeError ? 400 : 500;
    send(response, status, { error: status === 413 ? 'body_too_large' : status === 400 ? 'invalid_entry' : 'internal_error' });
  }
});

const port = Number.parseInt(process.env.PORT ?? '3000', 10);
server.listen(port, '127.0.0.1', () => {
  const address = server.address();
  console.log(JSON.stringify({ ready: true, port: address.port }));
});
"""

LEDGER_TEST = """import assert from 'node:assert/strict';
import test from 'node:test';
import { createLedger } from '../ledger.mjs';

test('ledger validates entries and produces exact category totals', () => {
  const ledger = createLedger();
  assert.deepEqual(ledger.summary(), { count: 0, totalCents: 0, byCategory: {} });
  assert.deepEqual(ledger.add({ amountCents: 1250, category: ' supplies ', note: 'paper' }), {
    id: 1,
    amountCents: 1250,
    category: 'supplies',
    note: 'paper',
  });
  ledger.add({ amountCents: 750, category: 'travel' });
  assert.deepEqual(ledger.summary(), {
    count: 2,
    totalCents: 2000,
    byCategory: { supplies: 1250, travel: 750 },
  });
  assert.throws(() => ledger.add({ amountCents: -1, category: 'bad' }), /positive integer/);
  assert.throws(() => ledger.add({ amountCents: 1, category: ' '.repeat(4) }), /category/);
});
"""


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


class ProviderServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self) -> None:
        super().__init__(("127.0.0.1", 0), ProviderHandler)
        self.call_count = 0
        self.call_lock = threading.Lock()


class ProviderHandler(BaseHTTPRequestHandler):
    server: ProviderServer

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def send_json(self, status: int, value: object) -> None:
        body = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path != "/v1/models":
            self.send_json(404, {"error": "not found"})
            return
        self.send_json(
            200,
            {
                "object": "list",
                "data": [
                    {
                        "id": MODEL,
                        "object": "model",
                        "owned_by": "mcg-installed-real-api-project-fixture",
                        "context_window": 32768,
                        "supported_parameters": ["tools"],
                    }
                ],
            },
        )

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/v1/responses":
            self.send_json(404, {"error": "not found"})
            return
        length = int(self.headers.get("Content-Length", "0"))
        request = json.loads(self.rfile.read(length))
        require(request.get("model") == MODEL, "fixture received the wrong model")
        with self.server.call_lock:
            self.server.call_count += 1
            call_number = self.server.call_count
        inputs = request.get("input") or []
        tools = request.get("tools") or []
        has_tool_result = any(item.get("type") == "function_call_output" for item in inputs)
        has_write_tool = any(
            (tool.get("name") or (tool.get("function") or {}).get("name")) == "fs.write"
            for tool in tools
        )
        if has_tool_result:
            response_id = "installed-ledger-complete"
            output: list[dict[str, Any]] = [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Pocket Ledger was built with governed writes and is ready to test.",
                        }
                    ],
                }
            ]
        elif has_write_tool:
            response_id = "installed-ledger-tools"
            output = [
                {
                    "type": "function_call",
                    "call_id": call_id,
                    "name": "fs.write",
                    "arguments": json.dumps(
                        {"path": path, "content": content, "create_new": True},
                        separators=(",", ":"),
                    ),
                }
                for call_id, path, content in [
                    ("write-ledger", "ledger.mjs", LEDGER_JS),
                    ("write-server", "server.mjs", SERVER_JS),
                    ("write-tests", "tests/ledger.test.mjs", LEDGER_TEST),
                ]
            ]
        else:
            response_id = "installed-ledger-evaluation"
            output = [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Independent evaluation reviewed the ledger build, test, and runtime evidence.",
                        }
                    ],
                }
            ]
        self.send_json(
            200,
            {
                "id": response_id,
                "model": MODEL,
                "status": "completed",
                "output": output,
                "usage": {"input_tokens": 96, "output_tokens": 48, "total_tokens": 144},
                "fixture_call": call_number,
            },
        )


def api_json(host: str, token: str, method: str, path: str, body: object | None = None) -> Any:
    data = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(
        f"{host.rstrip('/')}{path}",
        data=data,
        headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = response.read()
    except urllib.error.HTTPError as error:
        detail = error.read(4096).decode(errors="replace")
        raise AssertionError(f"{method} {path} failed with HTTP {error.code}: {detail}") from error
    return None if not payload else json.loads(payload)


def invoke_cli(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    evidence_root: pathlib.Path,
    label: str,
    arguments: list[str],
) -> subprocess.CompletedProcess[bytes]:
    result = subprocess.run(
        [
            str(cli),
            "--host",
            host,
            "--token-file",
            str(token_file),
            "-C",
            str(project),
            *arguments,
        ],
        input=b"",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=300,
        check=False,
    )
    evidence_root.joinpath(f"{label}.stdout").write_bytes(result.stdout)
    evidence_root.joinpath(f"{label}.stderr").write_bytes(result.stderr)
    return result


def decode_json(result: subprocess.CompletedProcess[bytes], label: str) -> Any:
    try:
        return json.loads(result.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AssertionError(f"{label} did not return JSON: {result.stdout[:4096]!r}") from error


def npm_command() -> str:
    executable = shutil.which("npm.cmd" if os.name == "nt" else "npm") or shutil.which("npm")
    require(executable is not None, "npm is required for the representative API project")
    return executable


def node_command() -> str:
    executable = shutil.which("node.exe" if os.name == "nt" else "node") or shutil.which("node")
    require(executable is not None, "Node.js is required for the representative API project")
    return executable


def run_project_test(project: pathlib.Path, evidence_root: pathlib.Path, label: str) -> int:
    result = subprocess.run(
        [npm_command(), "test"],
        cwd=project,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=60,
        check=False,
    )
    evidence_root.joinpath(f"{label}.log").write_bytes(result.stdout)
    return result.returncode


def prepare_project(project: pathlib.Path) -> None:
    project.mkdir(parents=True, exist_ok=True)
    (project / "tests").mkdir(exist_ok=True)
    (project / "README.md").write_text("# Pocket Ledger\n", encoding="utf-8")
    (project / "package.json").write_text(
        json.dumps(
            {
                "name": "pocket-ledger-installed-uat",
                "private": True,
                "type": "module",
                "scripts": {
                    "start": "node server.mjs",
                    "test": "node --test tests/ledger.test.mjs",
                },
            },
            separators=(",", ":"),
        )
        + "\n",
        encoding="utf-8",
    )


def expected_digests() -> dict[str, str]:
    return {
        "ledger.mjs": sha256(LEDGER_JS.encode()),
        "server.mjs": sha256(SERVER_JS.encode()),
        "tests/ledger.test.mjs": sha256(LEDGER_TEST.encode()),
    }


def verify_files(project: pathlib.Path) -> dict[str, str]:
    actual = {name: sha256((project / name).read_bytes()) for name in expected_digests()}
    require(actual == expected_digests(), "generated API bytes do not match the reviewed fixture")
    return actual


def reserve_loopback_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as reservation:
        reservation.bind(("127.0.0.1", 0))
        return int(reservation.getsockname()[1])


def runtime_request(
    endpoint: str,
    method: str,
    path: str,
    expected_status: int,
    body: object | bytes | None = None,
) -> Any:
    if isinstance(body, bytes):
        data = body
    else:
        data = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(
        endpoint + path,
        data=data,
        headers={"Content-Type": "application/json"},
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            status = response.status
            payload = response.read()
    except urllib.error.HTTPError as error:
        status = error.code
        payload = error.read()
    require(status == expected_status, f"{method} {path} returned {status}, expected {expected_status}")
    return json.loads(payload)


def verify_runtime(project: pathlib.Path, evidence_root: pathlib.Path, label: str) -> dict[str, Any]:
    port = reserve_loopback_port()
    endpoint = f"http://127.0.0.1:{port}"
    environment = os.environ.copy()
    environment["PORT"] = str(port)
    process = subprocess.Popen(
        [node_command(), "server.mjs"],
        cwd=project,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    transcript: dict[str, Any] = {}
    try:
        deadline = time.monotonic() + 15
        while True:
            require(process.poll() is None, "representative API exited before becoming healthy")
            try:
                transcript["health"] = runtime_request(endpoint, "GET", "/health", 200)
                break
            except (urllib.error.URLError, ConnectionError):
                if time.monotonic() >= deadline:
                    raise AssertionError("representative API did not become healthy")
                time.sleep(0.05)
        transcript["first"] = runtime_request(
            endpoint,
            "POST",
            "/entries",
            201,
            {"amountCents": 1250, "category": "supplies", "note": "printer paper"},
        )
        transcript["second"] = runtime_request(
            endpoint,
            "POST",
            "/entries",
            201,
            {"amountCents": 750, "category": "travel"},
        )
        transcript["summary"] = runtime_request(endpoint, "GET", "/summary", 200)
        transcript["invalid"] = runtime_request(
            endpoint,
            "POST",
            "/entries",
            400,
            {"amountCents": -1, "category": "invalid"},
        )
        transcript["oversized"] = runtime_request(
            endpoint,
            "POST",
            "/entries",
            413,
            b'{"note":"' + (b"x" * (33 * 1024)) + b'"}',
        )
        transcript["missing"] = runtime_request(endpoint, "GET", "/missing", 404)
        require(transcript["health"] == {"status": "ok"}, "runtime health response changed")
        require(transcript["first"]["entry"]["id"] == 1, "first runtime entry identity changed")
        require(transcript["second"]["entry"]["id"] == 2, "second runtime entry identity changed")
        require(
            transcript["summary"]
            == {"count": 2, "totalCents": 2000, "byCategory": {"supplies": 1250, "travel": 750}},
            "runtime summary changed",
        )
        require(transcript["invalid"] == {"error": "invalid_entry"}, "invalid entry did not fail closed")
        require(transcript["oversized"] == {"error": "body_too_large"}, "oversized body did not fail closed")
        require(transcript["missing"] == {"error": "not_found"}, "unknown route did not fail closed")
        report = {
            "schema_version": SCHEMA_VERSION,
            "loopback_only": True,
            "health_passed": True,
            "mutation_passed": True,
            "summary_passed": True,
            "invalid_input_failed_closed": True,
            "oversized_body_failed_closed": True,
            "unknown_route_failed_closed": True,
            "transcript_sha256": sha256(json.dumps(transcript, sort_keys=True).encode()),
        }
        evidence_root.joinpath(f"{label}.json").write_text(
            json.dumps(report, sort_keys=True),
            encoding="utf-8",
        )
        return report
    finally:
        process.terminate()
        try:
            stdout, stderr = process.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            stdout, stderr = process.communicate(timeout=10)
        evidence_root.joinpath(f"{label}.stdout").write_bytes(stdout)
        evidence_root.joinpath(f"{label}.stderr").write_bytes(stderr)
        require(process.returncode is not None, "representative API process did not terminate")
        try:
            runtime_request(endpoint, "GET", "/health", 200)
        except urllib.error.URLError:
            pass
        else:
            raise AssertionError("representative API listener remained after shutdown")


def task_audit(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    evidence_root: pathlib.Path,
    task_id: str,
    label: str,
) -> list[dict[str, Any]]:
    result = invoke_cli(cli, host, token_file, project, evidence_root, label, ["task", "audit", task_id])
    require(result.returncode == 0, f"{label} failed: {result.stderr[:4096]!r}")
    records = decode_json(result, label)["records"]
    completions = [
        record
        for record in records
        if record.get("phase") == "completion" and record.get("result") == "succeeded"
    ]
    require(sum(record.get("tool") == "fs.write" for record in completions) == 3, "API lacks three governed writes")
    require(any(record.get("tool") == "process.exec" for record in completions), "API test was not governed")
    return records


def exercise(args: argparse.Namespace) -> None:
    cli = pathlib.Path(args.cli).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    project = pathlib.Path(args.project).resolve()
    state_path = pathlib.Path(args.state).resolve()
    output_path = pathlib.Path(args.output).resolve()
    evidence_root = output_path.parent
    evidence_root.mkdir(parents=True, exist_ok=True)
    prepare_project(project)
    require(run_project_test(project, evidence_root, "real-api-before") != 0, "seeded API test unexpectedly passed")
    token = token_file.read_text(encoding="utf-8").strip()
    require(bool(token), "installed token is empty")

    provider = ProviderServer()
    provider_thread = threading.Thread(target=provider.serve_forever, daemon=True)
    provider_thread.start()
    provider_id = f"installed_real_api_{uuid.uuid4().hex}"
    configured = False
    try:
        mutation = api_json(
            args.host,
            token,
            "POST",
            "/api/v1/providers",
            {
                "connection": {
                    "contract_version": 1,
                    "id": provider_id,
                    "display_name": "Installed real API fixture",
                    "kind": "open_ai_compatible",
                    "base_url": f"http://127.0.0.1:{provider.server_address[1]}/v1",
                    "auth_mode": "none",
                    "organization": None,
                    "project": None,
                },
                "credential": None,
            },
        )
        configured = True
        require(mutation["catalog"]["models"][0]["id"] == MODEL, "fixture model was not discovered")
        result = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "real-api-run",
            ["run", "--json", "Build", "and", "verify", "the", "Pocket", "Ledger", "local", "JSON", "API"],
        )
        require(result.returncode == 0, f"real API project failed: {result.stderr[:4096]!r}")
        value = decode_json(result, "real-api-run")
        require(value["task"]["state"] == "completed", "real API task did not complete")
        require(value["run"]["run"]["state"] == "completed", "real API run did not complete")
        require(value["run"]["evaluations"][0]["outcome"] == "passed", "independent evaluation did not pass")
        require(len(value["run"]["results"]) == 2, "worker and evaluator results were not retained")
        require(provider.call_count == 3, f"expected three provider calls, observed {provider.call_count}")
        require(run_project_test(project, evidence_root, "real-api-after") == 0, "generated API regression test failed")
        digests = verify_files(project)
        task_id = value["task"]["id"]
        run_id = value["run"]["run"]["id"]
        audit = task_audit(cli, args.host, token_file, project, evidence_root, task_id, "real-api-audit")
        runtime = verify_runtime(project, evidence_root, "real-api-runtime")
        replay = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "real-api-replay",
            ["run", "replay", run_id, "--json"],
        )
        require(replay.returncode == 0, f"real API replay failed: {replay.stderr[:4096]!r}")
        replay_value = decode_json(replay, "real-api-replay")
        require(replay_value["side_effect_free"] is True, "real API replay was not side effect free")
        require(all(item["passed"] for item in replay_value["invariants"]), "real API replay invariant failed")
        require(provider.call_count == 3, "real API replay contacted the provider")
        require(token.encode() not in result.stdout and token.encode() not in result.stderr, "CLI exposed its token")
        state_path.write_text(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "task_id": task_id,
                    "run_id": run_id,
                    "project_path_sha256": sha256(str(project).encode()),
                    "files": digests,
                    "audit_count": len(audit),
                    "runtime_transcript_sha256": runtime["transcript_sha256"],
                    "provider_calls": provider.call_count,
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
        output_path.write_text(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "mode": "exercise",
                    "seeded_regression_failed": True,
                    "governed_files_written": 3,
                    "generated_regression_passed": True,
                    "independent_evaluation_passed": True,
                    "runtime_http_passed": True,
                    "loopback_only": True,
                    "invalid_input_failed_closed": True,
                    "oversized_body_failed_closed": True,
                    "provider_calls": provider.call_count,
                    "replay_side_effect_free": True,
                    "token_excluded": True,
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
    finally:
        if configured:
            api_json(args.host, token, "DELETE", f"/api/v1/providers/{provider_id}")
        provider.shutdown()
        provider.server_close()
        provider_thread.join(timeout=5)


def verify(args: argparse.Namespace) -> None:
    cli = pathlib.Path(args.cli).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    project = pathlib.Path(args.project).resolve(strict=True)
    state_path = pathlib.Path(args.state).resolve(strict=True)
    output_path = pathlib.Path(args.output).resolve()
    evidence_root = output_path.parent
    state = json.loads(state_path.read_text(encoding="utf-8"))
    require(state["schema_version"] == SCHEMA_VERSION, "real API state schema changed")
    require(state["project_path_sha256"] == sha256(str(project).encode()), "real API project path changed")
    require(verify_files(project) == state["files"], "real API files changed across restart")
    require(run_project_test(project, evidence_root, "real-api-restart-test") == 0, "real API restart regression failed")
    task = invoke_cli(
        cli,
        args.host,
        token_file,
        project,
        evidence_root,
        "real-api-restart-task",
        ["task", "get", state["task_id"], "--json"],
    )
    require(
        task.returncode == 0 and decode_json(task, "real-api-restart-task")["state"] == "completed",
        "real API task did not survive restart",
    )
    audit = task_audit(cli, args.host, token_file, project, evidence_root, state["task_id"], "real-api-restart-audit")
    require(len(audit) == state["audit_count"], "real API audit changed across restart")
    replay = invoke_cli(
        cli,
        args.host,
        token_file,
        project,
        evidence_root,
        "real-api-restart-replay",
        ["run", "replay", state["run_id"], "--json"],
    )
    require(replay.returncode == 0, f"real API restart replay failed: {replay.stderr[:4096]!r}")
    replay_value = decode_json(replay, "real-api-restart-replay")
    require(replay_value["side_effect_free"] is True, "real API restart replay was not side effect free")
    require(all(item["passed"] for item in replay_value["invariants"]), "real API restart replay invariant failed")
    runtime = verify_runtime(project, evidence_root, "real-api-restart-runtime")
    require(runtime["transcript_sha256"] == state["runtime_transcript_sha256"], "runtime behavior changed across restart")
    token = token_file.read_text(encoding="utf-8").strip().encode()
    require(token not in task.stdout and token not in task.stderr, "restart output exposed its token")
    output_path.write_text(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "mode": "verify",
                "daemon_restart_survived": True,
                "files_unchanged": True,
                "generated_regression_passed": True,
                "audit_unchanged": True,
                "provider_required_for_replay": False,
                "runtime_http_passed": True,
                "runtime_behavior_unchanged": True,
                "loopback_only": True,
                "invalid_input_failed_closed": True,
                "oversized_body_failed_closed": True,
                "token_excluded": True,
            },
            sort_keys=True,
        ),
        encoding="utf-8",
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["exercise", "verify"], required=True)
    parser.add_argument("--host", required=True)
    parser.add_argument("--token-file", required=True)
    parser.add_argument("--cli", required=True)
    parser.add_argument("--project", required=True)
    parser.add_argument("--state", required=True)
    parser.add_argument("--output", required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.mode == "exercise":
        exercise(args)
    else:
        verify(args)


if __name__ == "__main__":
    main()
