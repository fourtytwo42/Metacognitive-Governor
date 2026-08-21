#!/usr/bin/env python3
"""Build and verify a representative full-stack consumer app through installed MCG."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


SCHEMA_VERSION = 1
MODEL = "installed-real-fullstack-project-model"

INDEX_HTML = """<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>HomeBase Inventory</title>
  <link rel="stylesheet" href="/styles.css">
</head>
<body>
  <main>
    <p class="eyebrow">HomeBase</p>
    <h1>Know what you have and where it lives.</h1>
    <form id="item-form">
      <label for="name">Item</label><input id="name" name="name" required maxlength="60">
      <label for="quantity">Quantity</label><input id="quantity" name="quantity" type="number" min="1" max="999" value="1" required>
      <label for="location">Location</label><input id="location" name="location" required maxlength="40">
      <button type="submit">Add item</button>
    </form>
    <p id="status" role="status" aria-live="polite">Loading inventory…</p>
    <ul id="items" aria-label="Inventory items"></ul>
  </main>
  <script type="module" src="/app.js"></script>
</body>
</html>
"""

STYLES_CSS = """:root { color-scheme: light; font-family: system-ui, sans-serif; background: #f2f5f4; color: #17211d; }
body { margin: 0; min-height: 100vh; display: grid; place-items: center; }
main { width: min(46rem, calc(100% - 2rem)); padding: clamp(1.25rem, 5vw, 3rem); background: white; border-radius: 1.25rem; box-shadow: 0 1rem 3rem #1733261f; }
.eyebrow { color: #176943; font-weight: 800; letter-spacing: .09em; text-transform: uppercase; }
h1 { max-width: 18ch; line-height: 1.05; }
form { display: grid; grid-template-columns: auto 1fr; gap: .65rem 1rem; align-items: center; }
input, button { font: inherit; padding: .7rem; border-radius: .55rem; }
input { border: 2px solid #91a39a; }
button { grid-column: 2; border: 0; background: #185f40; color: white; font-weight: 750; cursor: pointer; }
button:focus-visible, input:focus-visible { outline: 3px solid #e39a22; outline-offset: 2px; }
li { margin-block: .6rem; padding: .75rem; background: #edf5f0; border-radius: .55rem; }
@media (max-width: 36rem) { main { border-radius: 0; } form { grid-template-columns: 1fr; } button { grid-column: 1; } }
"""

APP_JS = """const form = document.querySelector('#item-form');
const list = document.querySelector('#items');
const status = document.querySelector('#status');

function render(items) {
  list.replaceChildren(...items.map((item) => {
    const row = document.createElement('li');
    row.textContent = `${item.quantity} × ${item.name} — ${item.location}`;
    return row;
  }));
  status.textContent = `${items.length} item${items.length === 1 ? '' : 's'} recorded.`;
}

async function load() {
  const response = await fetch('/api/items');
  if (!response.ok) throw new Error('Inventory could not be loaded.');
  render((await response.json()).items);
}

form.addEventListener('submit', async (event) => {
  event.preventDefault();
  const body = Object.fromEntries(new FormData(form));
  body.quantity = Number(body.quantity);
  const response = await fetch('/api/items', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
  });
  if (!response.ok) { status.textContent = 'Check the item details and try again.'; return; }
  form.reset();
  document.querySelector('#quantity').value = '1';
  await load();
  document.querySelector('#name').focus();
});

await load();
const quickAdd = new URLSearchParams(location.search);
if (quickAdd.has('item')) {
  document.querySelector('#name').value = quickAdd.get('item');
  document.querySelector('#quantity').value = quickAdd.get('quantity') ?? '1';
  document.querySelector('#location').value = quickAdd.get('location') ?? 'Unsorted';
  form.requestSubmit();
}
"""

SERVER_MJS = """import http from 'node:http';
import { mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const statePath = join(root, 'data', 'items.json');
const MAX_BODY_BYTES = 32 * 1024;

async function readItems() {
  try { const value = JSON.parse(await readFile(statePath, 'utf8')); return Array.isArray(value) ? value : []; }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
async function saveItems(items) {
  await mkdir(dirname(statePath), { recursive: true });
  const temporary = `${statePath}.tmp`;
  await writeFile(temporary, `${JSON.stringify(items)}\n`, { encoding: 'utf8', flag: 'w' });
  await rename(temporary, statePath);
}
function send(response, status, type, body) {
  response.writeHead(status, { 'content-type': type, 'cache-control': 'no-store', 'content-length': Buffer.byteLength(body), 'x-content-type-options': 'nosniff' });
  response.end(body);
}
function sendJson(response, status, value) { send(response, status, 'application/json; charset=utf-8', JSON.stringify(value)); }
async function readJson(request) {
  const chunks = []; let length = 0;
  for await (const chunk of request) { length += chunk.length; if (length > MAX_BODY_BYTES) { const error = new Error(); error.status = 413; throw error; } chunks.push(chunk); }
  try { return JSON.parse(Buffer.concat(chunks).toString('utf8')); } catch { const error = new Error(); error.status = 400; throw error; }
}
function validate(input) {
  const name = typeof input?.name === 'string' ? input.name.trim() : '';
  const location = typeof input?.location === 'string' ? input.location.trim() : '';
  if (!name || name.length > 60 || !location || location.length > 40 || !Number.isInteger(input?.quantity) || input.quantity < 1 || input.quantity > 999) throw new TypeError();
  return { name, quantity: input.quantity, location };
}
const assets = new Map([
  ['/', ['public/index.html', 'text/html; charset=utf-8']], ['/index.html', ['public/index.html', 'text/html; charset=utf-8']],
  ['/styles.css', ['public/styles.css', 'text/css; charset=utf-8']], ['/app.js', ['public/app.js', 'text/javascript; charset=utf-8']],
]);
export const server = http.createServer(async (request, response) => {
  try {
    const path = new URL(request.url, 'http://127.0.0.1').pathname;
    if (request.method === 'GET' && path === '/health') { sendJson(response, 200, { status: 'ok' }); return; }
    if (request.method === 'GET' && path === '/api/items') { sendJson(response, 200, { items: await readItems() }); return; }
    if (request.method === 'POST' && path === '/api/items') {
      const item = validate(await readJson(request)); const items = await readItems();
      const created = { id: items.length + 1, ...item }; items.push(created); await saveItems(items);
      sendJson(response, 201, { item: created }); return;
    }
    if (request.method === 'GET' && assets.has(path)) { const [file, type] = assets.get(path); send(response, 200, type, await readFile(join(root, file))); return; }
    sendJson(response, 404, { error: 'not_found' });
  } catch (error) { const status = error.status === 413 ? 413 : error instanceof TypeError || error.status === 400 ? 400 : 500; sendJson(response, status, { error: status === 413 ? 'body_too_large' : status === 400 ? 'invalid_item' : 'internal_error' }); }
});
const port = Number.parseInt(process.env.PORT ?? '3000', 10);
server.listen(port, '127.0.0.1', () => console.log(JSON.stringify({ ready: true, port: server.address().port })));
"""

APP_TEST = r"""import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

test('HomeBase has accessible full-stack source and bounded persistence', async () => {
  const [html, css, app, server] = await Promise.all(['public/index.html', 'public/styles.css', 'public/app.js', 'server.mjs'].map((file) => readFile(new URL(`../${file}`, import.meta.url), 'utf8')));
  assert.match(html, /<html lang="en">/); assert.match(html, /role="status"/); assert.match(html, /aria-label="Inventory items"/);
  assert.match(css, /:focus-visible/); assert.match(css, /@media \(max-width: 36rem\)/);
  assert.match(app, /form\.addEventListener\('submit'/); assert.match(app, /fetch\('\/api\/items'/); assert.match(app, /form\.requestSubmit\(\)/);
  assert.match(server, /MAX_BODY_BYTES = 32 \* 1024/); assert.match(server, /await rename\(temporary, statePath\)/); assert.match(server, /127\.0\.0\.1/);
});
"""


def load_shared() -> Any:
    path = pathlib.Path(__file__).with_name("test-installed-real-api-project.py")
    spec = importlib.util.spec_from_file_location("mcg_real_api_shared", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SHARED = load_shared()
sha256 = SHARED.sha256
require = SHARED.require
api_json = SHARED.api_json
invoke_cli = SHARED.invoke_cli
decode_json = SHARED.decode_json
npm_command = SHARED.npm_command
node_command = SHARED.node_command
reserve_loopback_port = SHARED.reserve_loopback_port
runtime_request = SHARED.runtime_request


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

    def send_json(self, value: object) -> None:
        body = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path != "/v1/models":
            self.send_error(404)
            return
        self.send_json(
            {
                "object": "list",
                "data": [
                    {
                        "id": MODEL,
                        "object": "model",
                        "owned_by": "mcg-installed-fullstack-fixture",
                        "context_window": 32768,
                        "supported_parameters": ["tools"],
                    }
                ],
            }
        )

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/v1/responses":
            self.send_error(404)
            return
        request = json.loads(
            self.rfile.read(int(self.headers.get("Content-Length", "0")))
        )
        require(request.get("model") == MODEL, "fixture received the wrong model")
        with self.server.call_lock:
            self.server.call_count += 1
            call_number = self.server.call_count
        inputs = request.get("input") or []
        tools = request.get("tools") or []
        has_result = any(item.get("type") == "function_call_output" for item in inputs)
        has_write = any(
            (tool.get("name") or (tool.get("function") or {}).get("name")) == "fs.write"
            for tool in tools
        )
        if has_result:
            response_id = "installed-homebase-complete"
            output = [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "HomeBase was built with governed writes and is ready to test.",
                        }
                    ],
                }
            ]
        elif has_write:
            response_id = "installed-homebase-tools"
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
                    ("write-server", "server.mjs", SERVER_MJS),
                    ("write-index", "public/index.html", INDEX_HTML),
                    ("write-styles", "public/styles.css", STYLES_CSS),
                    ("write-app", "public/app.js", APP_JS),
                    ("write-tests", "tests/app.test.mjs", APP_TEST),
                ]
            ]
        else:
            response_id = "installed-homebase-evaluation"
            output = [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Independent evaluation reviewed the full-stack build, regression, browser, persistence, and API evidence.",
                        }
                    ],
                }
            ]
        self.send_json(
            {
                "id": response_id,
                "model": MODEL,
                "status": "completed",
                "output": output,
                "usage": {
                    "input_tokens": 128,
                    "output_tokens": 64,
                    "total_tokens": 192,
                },
                "fixture_call": call_number,
            }
        )


def prepare_project(project: pathlib.Path) -> None:
    project.mkdir(parents=True, exist_ok=True)
    (project / "public").mkdir(exist_ok=True)
    (project / "tests").mkdir(exist_ok=True)
    (project / "README.md").write_text("# HomeBase Inventory\n", encoding="utf-8")
    (project / "package.json").write_text(
        json.dumps(
            {
                "name": "homebase-installed-uat",
                "private": True,
                "type": "module",
                "scripts": {
                    "start": "node server.mjs",
                    "test": "node --test tests/app.test.mjs",
                },
            },
            separators=(",", ":"),
        )
        + "\n",
        encoding="utf-8",
    )


def expected_digests() -> dict[str, str]:
    return {
        name: hashlib.sha256(content.encode()).hexdigest()
        for name, content in {
            "server.mjs": SERVER_MJS,
            "public/index.html": INDEX_HTML,
            "public/styles.css": STYLES_CSS,
            "public/app.js": APP_JS,
            "tests/app.test.mjs": APP_TEST,
        }.items()
    }


def verify_files(project: pathlib.Path) -> dict[str, str]:
    actual = {
        name: sha256((project / name).read_bytes()) for name in expected_digests()
    }
    require(
        actual == expected_digests(),
        "generated full-stack bytes do not match reviewed fixture",
    )
    return actual


def run_test(project: pathlib.Path, evidence: pathlib.Path, label: str) -> int:
    result = subprocess.run(
        [npm_command(), "test"],
        cwd=project,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=60,
        check=False,
    )
    evidence.joinpath(f"{label}.log").write_bytes(result.stdout)
    return result.returncode


def browser_executable() -> pathlib.Path:
    configured = os.environ.get("MCG_BROWSER_EXECUTABLE")
    candidates = [pathlib.Path(configured)] if configured else []
    if sys.platform == "darwin":
        candidates += list(
            pathlib.Path.home().glob(
                "Library/Caches/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-mac-*/chrome-headless-shell"
            )
        )
    elif os.name == "nt":
        candidates += list(
            pathlib.Path.home().glob(
                "AppData/Local/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-win*/chrome-headless-shell.exe"
            )
        )
    else:
        candidates += list(
            pathlib.Path.home().glob(
                ".cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux*/chrome-headless-shell"
            )
        ) + [pathlib.Path("/usr/bin/chromium"), pathlib.Path("/usr/bin/google-chrome")]
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise AssertionError("no installed Chromium-family browser was found")


def launch_app(project: pathlib.Path, port: int) -> subprocess.Popen[bytes]:
    environment = os.environ.copy()
    environment["PORT"] = str(port)
    return subprocess.Popen(
        [node_command(), "server.mjs"],
        cwd=project,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def wait_healthy(process: subprocess.Popen[bytes], endpoint: str) -> None:
    deadline = time.monotonic() + 15
    while True:
        require(process.poll() is None, "HomeBase exited before becoming healthy")
        try:
            require(
                runtime_request(endpoint, "GET", "/health", 200) == {"status": "ok"},
                "HomeBase health changed",
            )
            return
        except (urllib.error.URLError, ConnectionError):
            if time.monotonic() >= deadline:
                raise AssertionError("HomeBase did not become healthy")
            time.sleep(0.05)


def stop_app(
    process: subprocess.Popen[bytes], endpoint: str, evidence: pathlib.Path, label: str
) -> None:
    process.terminate()
    try:
        stdout, stderr = process.communicate(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        stdout, stderr = process.communicate(timeout=10)
    evidence.joinpath(f"{label}.stdout").write_bytes(stdout)
    evidence.joinpath(f"{label}.stderr").write_bytes(stderr)
    try:
        runtime_request(endpoint, "GET", "/health", 200)
    except urllib.error.URLError:
        return
    raise AssertionError("HomeBase listener remained after shutdown")


def browser_run(
    url: str, evidence: pathlib.Path, label: str, screenshot: bool
) -> tuple[str, bytes | None]:
    profile = pathlib.Path(tempfile.mkdtemp(prefix="mcg-homebase-browser-"))
    image = evidence / f"{label}.png"
    try:
        base = [
            str(browser_executable()),
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--no-first-run",
            "--no-default-browser-check",
            f"--user-data-dir={profile}",
            "--window-size=1280,720",
            "--virtual-time-budget=3000",
        ]
        if sys.platform.startswith("linux"):
            base.insert(1, "--no-sandbox")
        captured = subprocess.run(
            [*base, f"--screenshot={image}", url],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
        require(
            captured.returncode == 0,
            f"HomeBase browser screenshot failed: {captured.stderr[:4096]!r}",
        )
        if not screenshot:
            return "", None
        dom = subprocess.run(
            [*base, "--dump-dom", url],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
        require(
            dom.returncode == 0, f"HomeBase DOM capture failed: {dom.stderr[:4096]!r}"
        )
        return dom.stdout.decode(
            "utf-8", errors="replace"
        ), image.read_bytes() if screenshot else None
    finally:
        shutil.rmtree(profile, ignore_errors=True)


def verify_runtime(
    project: pathlib.Path, evidence: pathlib.Path, label: str
) -> dict[str, Any]:
    data = (project / "data").resolve()
    require(data.parent == project.resolve(), "HomeBase data escaped project")
    if data.exists():
        shutil.rmtree(data)
    port = reserve_loopback_port()
    endpoint = f"http://127.0.0.1:{port}"
    first = launch_app(project, port)
    try:
        wait_healthy(first, endpoint)
        quick = urllib.parse.urlencode(
            {"item": "Desk lamp", "quantity": "2", "location": "Office"}
        )
        browser_run(f"{endpoint}/?{quick}", evidence, f"{label}-quick-add", False)
        deadline = time.monotonic() + 10
        while True:
            items = runtime_request(endpoint, "GET", "/api/items", 200)
            if items.get("items"):
                break
            require(
                time.monotonic() < deadline, "browser quick-add did not reach the API"
            )
            time.sleep(0.05)
        require(
            items
            == {
                "items": [
                    {"id": 1, "name": "Desk lamp", "quantity": 2, "location": "Office"}
                ]
            },
            "browser-created inventory changed",
        )
        require(
            runtime_request(
                endpoint,
                "POST",
                "/api/items",
                400,
                {"name": "", "quantity": 0, "location": ""},
            )
            == {"error": "invalid_item"},
            "invalid item did not fail closed",
        )
        require(
            runtime_request(
                endpoint,
                "POST",
                "/api/items",
                413,
                b'{"name":"' + b"x" * (33 * 1024) + b'"}',
            )
            == {"error": "body_too_large"},
            "oversized body did not fail closed",
        )
        require(
            runtime_request(endpoint, "GET", "/missing", 404) == {"error": "not_found"},
            "unknown route did not fail closed",
        )
    finally:
        stop_app(first, endpoint, evidence, f"{label}-first-server")
    second = launch_app(project, port)
    try:
        wait_healthy(second, endpoint)
        require(
            runtime_request(endpoint, "GET", "/api/items", 200) == items,
            "inventory did not survive application restart",
        )
        dom, png = browser_run(f"{endpoint}/", evidence, f"{label}-restart", True)
        require(
            "HomeBase Inventory" in dom
            and "2 × Desk lamp — Office" in dom
            and "1 item recorded." in dom,
            "restarted browser UI did not render persisted inventory",
        )
        assert png is not None
        require(
            png.startswith(b"\x89PNG\r\n\x1a\n") and len(png) > 4096,
            "HomeBase screenshot is invalid",
        )
        width, height = struct.unpack(">II", png[16:24])
        require(
            (width, height) == (1280, 720), "HomeBase screenshot dimensions changed"
        )
    finally:
        stop_app(second, endpoint, evidence, f"{label}-restart-server")
    report = {
        "schema_version": SCHEMA_VERSION,
        "loopback_only": True,
        "browser_quick_add_passed": True,
        "persistent_after_app_restart": True,
        "invalid_input_failed_closed": True,
        "oversized_body_failed_closed": True,
        "unknown_route_failed_closed": True,
        "listener_teardown_passed": True,
        "viewport": [1280, 720],
    }
    report["transcript_sha256"] = sha256(json.dumps(report, sort_keys=True).encode())
    evidence.joinpath(f"{label}.json").write_text(
        json.dumps(report, sort_keys=True), encoding="utf-8"
    )
    return report


def task_audit(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    evidence: pathlib.Path,
    task_id: str,
    label: str,
) -> list[dict[str, Any]]:
    result = invoke_cli(
        cli, host, token_file, project, evidence, label, ["task", "audit", task_id]
    )
    require(result.returncode == 0, f"{label} failed: {result.stderr[:4096]!r}")
    records = decode_json(result, label)["records"]
    completions = [
        record
        for record in records
        if record.get("phase") == "completion" and record.get("result") == "succeeded"
    ]
    require(
        sum(record.get("tool") == "fs.write" for record in completions) == 5,
        "HomeBase lacks five governed writes",
    )
    require(
        any(record.get("tool") == "process.exec" for record in completions),
        "HomeBase test was not governed",
    )
    return records


def exercise(args: argparse.Namespace) -> None:
    cli = pathlib.Path(args.cli).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    project = pathlib.Path(args.project).resolve()
    state_path = pathlib.Path(args.state).resolve()
    output = pathlib.Path(args.output).resolve()
    evidence = output.parent
    evidence.mkdir(parents=True, exist_ok=True)
    prepare_project(project)
    require(
        run_test(project, evidence, "real-fullstack-before") != 0,
        "seeded HomeBase test unexpectedly passed",
    )
    token = token_file.read_text(encoding="utf-8").strip()
    require(bool(token), "installed token is empty")
    provider = ProviderServer()
    thread = threading.Thread(target=provider.serve_forever, daemon=True)
    thread.start()
    provider_id = f"installed_real_fullstack_{uuid.uuid4().hex}"
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
                    "display_name": "Installed HomeBase fixture",
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
        require(
            mutation["catalog"]["models"][0]["id"] == MODEL,
            "fixture model was not discovered",
        )
        result = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence,
            "real-fullstack-run",
            [
                "run",
                "--json",
                "Build",
                "and",
                "verify",
                "the",
                "HomeBase",
                "inventory",
                "application",
            ],
        )
        require(
            result.returncode == 0, f"HomeBase project failed: {result.stderr[:4096]!r}"
        )
        value = decode_json(result, "real-fullstack-run")
        require(
            value["task"]["state"] == "completed"
            and value["run"]["run"]["state"] == "completed",
            "HomeBase run did not complete",
        )
        require(
            value["run"]["evaluations"][0]["outcome"] == "passed"
            and len(value["run"]["results"]) == 2,
            "HomeBase independent evaluation was not retained",
        )
        require(
            provider.call_count == 3,
            f"expected three provider calls, observed {provider.call_count}",
        )
        require(
            run_test(project, evidence, "real-fullstack-after") == 0,
            "generated HomeBase regression failed",
        )
        digests = verify_files(project)
        task_id = value["task"]["id"]
        run_id = value["run"]["run"]["id"]
        audit = task_audit(
            cli,
            args.host,
            token_file,
            project,
            evidence,
            task_id,
            "real-fullstack-audit",
        )
        runtime = verify_runtime(project, evidence, "real-fullstack-runtime")
        replay = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence,
            "real-fullstack-replay",
            ["run", "replay", run_id, "--json"],
        )
        replay_value = decode_json(replay, "real-fullstack-replay")
        require(
            replay.returncode == 0
            and replay_value["side_effect_free"] is True
            and all(item["passed"] for item in replay_value["invariants"]),
            "HomeBase replay failed",
        )
        require(provider.call_count == 3, "HomeBase replay contacted provider")
        require(
            token.encode() not in result.stdout and token.encode() not in result.stderr,
            "CLI exposed its token",
        )
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
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
        output.write_text(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "mode": "exercise",
                    "seeded_regression_failed": True,
                    "governed_files_written": 5,
                    "generated_regression_passed": True,
                    "independent_evaluation_passed": True,
                    "browser_quick_add_passed": True,
                    "persistent_after_app_restart": True,
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
        thread.join(timeout=5)


def verify(args: argparse.Namespace) -> None:
    cli = pathlib.Path(args.cli).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    project = pathlib.Path(args.project).resolve(strict=True)
    state = json.loads(
        pathlib.Path(args.state).resolve(strict=True).read_text(encoding="utf-8")
    )
    output = pathlib.Path(args.output).resolve()
    evidence = output.parent
    require(
        state["schema_version"] == SCHEMA_VERSION
        and state["project_path_sha256"] == sha256(str(project).encode()),
        "HomeBase durable state changed",
    )
    require(
        verify_files(project) == state["files"],
        "HomeBase files changed across daemon restart",
    )
    require(
        run_test(project, evidence, "real-fullstack-restart-test") == 0,
        "HomeBase restart regression failed",
    )
    task = invoke_cli(
        cli,
        args.host,
        token_file,
        project,
        evidence,
        "real-fullstack-restart-task",
        ["task", "get", state["task_id"], "--json"],
    )
    require(
        task.returncode == 0
        and decode_json(task, "real-fullstack-restart-task")["state"] == "completed",
        "HomeBase task did not survive daemon restart",
    )
    audit = task_audit(
        cli,
        args.host,
        token_file,
        project,
        evidence,
        state["task_id"],
        "real-fullstack-restart-audit",
    )
    require(
        len(audit) == state["audit_count"],
        "HomeBase audit changed across daemon restart",
    )
    replay = invoke_cli(
        cli,
        args.host,
        token_file,
        project,
        evidence,
        "real-fullstack-restart-replay",
        ["run", "replay", state["run_id"], "--json"],
    )
    replay_value = decode_json(replay, "real-fullstack-restart-replay")
    require(
        replay.returncode == 0
        and replay_value["side_effect_free"] is True
        and all(item["passed"] for item in replay_value["invariants"]),
        "HomeBase daemon-restart replay failed",
    )
    runtime = verify_runtime(project, evidence, "real-fullstack-restart-runtime")
    require(
        runtime["transcript_sha256"] == state["runtime_transcript_sha256"],
        "HomeBase runtime changed across daemon restart",
    )
    token = token_file.read_text(encoding="utf-8").strip().encode()
    require(
        token not in task.stdout and token not in task.stderr,
        "restart output exposed token",
    )
    output.write_text(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "mode": "verify",
                "daemon_restart_survived": True,
                "files_unchanged": True,
                "audit_unchanged": True,
                "generated_regression_passed": True,
                "provider_required_for_replay": False,
                "browser_quick_add_passed": True,
                "persistent_after_app_restart": True,
                "runtime_behavior_unchanged": True,
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
    exercise(args) if args.mode == "exercise" else verify(args)


if __name__ == "__main__":
    main()
