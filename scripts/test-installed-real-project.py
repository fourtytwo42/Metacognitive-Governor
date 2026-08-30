#!/usr/bin/env python3
"""Build and verify a representative website through an installed MCG package."""

from __future__ import annotations

import argparse
import hashlib
import http.server
import json
import os
import pathlib
import shutil
import struct
import subprocess
import sys
import tempfile
import threading
import urllib.error
import urllib.parse
import urllib.request
import uuid
from html.parser import HTMLParser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


SCHEMA_VERSION = 1
MODEL = "installed-real-project-model"
TITLE = "LaunchPad"

INDEX_HTML = """<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>LaunchPad</title>
  <link rel="stylesheet" href="styles.css">
</head>
<body>
  <main id="planner">
    <p class="eyebrow">A tiny project planner</p>
    <h1>Turn today’s idea into a clear next step.</h1>
    <p><a href="#task-form">Skip to the planner</a></p>
    <form id="task-form">
      <label for="task">What do you want to ship?</label>
      <div class="controls">
        <input id="task" name="task" required maxlength="80" autocomplete="off">
        <button type="submit">Add next step</button>
      </div>
    </form>
    <p id="status" role="status" aria-live="polite">No steps yet.</p>
    <ul id="task-list" aria-label="Project steps"></ul>
  </main>
  <script src="app.js"></script>
</body>
</html>
"""

STYLES_CSS = """:root { color-scheme: light; font-family: system-ui, sans-serif; background: #f4f1ea; color: #17201b; }
body { margin: 0; min-height: 100vh; display: grid; place-items: center; }
main { width: min(42rem, calc(100% - 2rem)); background: #fffdf8; padding: clamp(1.5rem, 5vw, 3.5rem); border-radius: 1.5rem; box-shadow: 0 1rem 3rem #1833231f; }
.eyebrow { color: #24734b; font-weight: 700; text-transform: uppercase; letter-spacing: .08em; }
h1 { font-size: clamp(2rem, 6vw, 3.6rem); line-height: 1; max-width: 12ch; }
label { display: block; font-weight: 700; margin-bottom: .5rem; }
.controls { display: flex; gap: .6rem; flex-wrap: wrap; }
input { flex: 1 1 16rem; border: 2px solid #9aab9f; border-radius: .7rem; padding: .8rem; font: inherit; }
button { border: 0; border-radius: .7rem; padding: .8rem 1rem; background: #195f3d; color: white; font: inherit; font-weight: 700; cursor: pointer; }
button:focus-visible, input:focus-visible, a:focus-visible { outline: 3px solid #e39d27; outline-offset: 2px; }
li { margin-block: .7rem; padding: .8rem 1rem; background: #eef5ef; border-radius: .7rem; }
@media (max-width: 36rem) { main { border-radius: 0; } .controls > * { flex-basis: 100%; } }
"""

APP_JS = """const form = document.querySelector('#task-form');
const input = document.querySelector('#task');
const list = document.querySelector('#task-list');
const status = document.querySelector('#status');
form.addEventListener('submit', (event) => {
  event.preventDefault();
  const value = input.value.trim();
  if (!value) return;
  const item = document.createElement('li');
  item.textContent = value;
  list.append(item);
  input.value = '';
  status.textContent = `${list.children.length} step${list.children.length === 1 ? '' : 's'} ready.`;
  input.focus();
});
"""

WEBSITE_TEST = r"""import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

test('LaunchPad has responsive, accessible, interactive source', async () => {
  const [html, css, script] = await Promise.all([
    readFile(new URL('../index.html', import.meta.url), 'utf8'),
    readFile(new URL('../styles.css', import.meta.url), 'utf8'),
    readFile(new URL('../app.js', import.meta.url), 'utf8'),
  ]);
  assert.match(html, /<html lang="en">/);
  assert.match(html, /role="status"/);
  assert.match(html, /href="#task-form"/);
  assert.match(html, /styles\.css/);
  assert.match(html, /app\.js/);
  assert.match(css, /:focus-visible/);
  assert.match(css, /@media \(max-width: 36rem\)/);
  assert.match(css, /clamp\(/);
  assert.match(script, /addEventListener\('submit'/);
  assert.match(script, /document\.createElement\('li'\)/);
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
                        "owned_by": "mcg-installed-real-project-fixture",
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
        has_tool_result = any(
            item.get("type") == "function_call_output" for item in inputs
        )
        has_write_tool = any(
            (tool.get("name") or (tool.get("function") or {}).get("name")) == "fs.write"
            for tool in tools
        )
        if has_tool_result:
            response_id = "installed-website-complete"
            output: list[dict[str, Any]] = [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "LaunchPad was built with governed writes and is ready to test.",
                        }
                    ],
                }
            ]
        elif has_write_tool:
            response_id = "installed-website-tools"
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
                    ("write-index", "index.html", INDEX_HTML),
                    ("write-styles", "styles.css", STYLES_CSS),
                    ("write-script", "app.js", APP_JS),
                ]
            ]
        else:
            response_id = "installed-website-evaluation"
            output = [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Independent evaluation reviewed the governed write and test evidence.",
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


def api_json(
    host: str, token: str, method: str, path: str, body: object | None = None
) -> Any:
    data = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(
        f"{host.rstrip('/')}{path}",
        data=data,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
        },
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = response.read()
    except urllib.error.HTTPError as error:
        detail = error.read(4096).decode(errors="replace")
        raise AssertionError(
            f"{method} {path} failed with HTTP {error.code}: {detail}"
        ) from error
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
        raise AssertionError(
            f"{label} did not return JSON: {result.stdout[:4096]!r}"
        ) from error


def node_command() -> str:
    executable = shutil.which("node")
    require(
        executable is not None, "Node.js is required for the representative project"
    )
    return executable


def npm_command() -> str:
    executable = shutil.which("npm.cmd" if os.name == "nt" else "npm") or shutil.which(
        "npm"
    )
    require(executable is not None, "npm is required for the representative project")
    return executable


def run_project_test(
    project: pathlib.Path, evidence_root: pathlib.Path, label: str
) -> int:
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


def browser_candidates() -> list[pathlib.Path]:
    values: list[pathlib.Path] = []
    cache_homes = [pathlib.Path.home()]
    if os.name != "nt":
        import pwd

        account_home = pathlib.Path(pwd.getpwuid(os.getuid()).pw_dir)
        if account_home not in cache_homes:
            cache_homes.append(account_home)
    configured = os.environ.get("MCG_BROWSER_EXECUTABLE")
    if configured:
        values.append(pathlib.Path(configured))
    if sys.platform == "darwin":
        for cache_home in cache_homes:
            playwright_cache = cache_home / "Library/Caches/ms-playwright"
            values.extend(
                playwright_cache.glob(
                    "chromium_headless_shell-*/chrome-headless-shell-mac-*/chrome-headless-shell"
                )
            )
            values.extend(
                playwright_cache.glob(
                    "chromium-*/chrome-mac*/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
                )
            )
            values.extend(
                playwright_cache.glob(
                    "chromium-*/chrome-mac*/Chromium.app/Contents/MacOS/Chromium"
                )
            )
        values.extend(
            [
                pathlib.Path(
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
                ),
                pathlib.Path("/Applications/Chromium.app/Contents/MacOS/Chromium"),
            ]
        )
    elif os.name == "nt":
        values.extend(
            pathlib.Path.home().glob(
                "AppData/Local/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-win*/chrome-headless-shell.exe"
            )
        )
        for root in filter(
            None,
            [
                os.environ.get("PROGRAMFILES"),
                os.environ.get("PROGRAMFILES(X86)"),
                os.environ.get("LOCALAPPDATA"),
            ],
        ):
            values.append(pathlib.Path(root) / "Google/Chrome/Application/chrome.exe")
        values.extend(
            pathlib.Path.home().glob(
                "AppData/Local/ms-playwright/chromium-*/chrome-win/chrome.exe"
            )
        )
        values.extend(
            pathlib.Path.home().glob(
                "AppData/Local/ms-playwright/chromium-*/chrome-win64/chrome.exe"
            )
        )
    else:
        for cache_home in cache_homes:
            values.extend(
                cache_home.glob(
                    ".cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux*/chrome-headless-shell"
                )
            )
            values.extend(
                cache_home.glob(".cache/ms-playwright/chromium-*/chrome-linux*/chrome")
            )
        values.extend(
            pathlib.Path(path)
            for path in [
                "/usr/bin/google-chrome",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
            ]
        )
    return values


def browser_executable() -> pathlib.Path:
    for candidate in browser_candidates():
        if candidate.is_file():
            return candidate.resolve()
    raise AssertionError("no installed Chromium-family browser was found")


class LinkParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.references: list[str] = []

    def handle_starttag(
        self, _tag: str, attributes: list[tuple[str, str | None]]
    ) -> None:
        for name, value in attributes:
            if name in {"href", "src"} and value:
                self.references.append(value)


def render_project(
    project: pathlib.Path, evidence_root: pathlib.Path, label: str
) -> dict[str, Any]:
    class QuietHandler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *arguments: Any, **kwargs: Any) -> None:
            super().__init__(*arguments, directory=str(project), **kwargs)

        def log_message(self, _format: str, *_arguments: object) -> None:
            return

    server = ThreadingHTTPServer(("127.0.0.1", 0), QuietHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    url = f"http://127.0.0.1:{server.server_address[1]}/index.html"
    profile = pathlib.Path(tempfile.mkdtemp(prefix="mcg-real-project-browser-"))
    screenshot = evidence_root / f"{label}-browser.png"
    try:
        command = [
            str(browser_executable()),
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--no-first-run",
            "--no-default-browser-check",
            f"--user-data-dir={profile}",
            "--window-size=1280,720",
            "--virtual-time-budget=1000",
            f"--screenshot={screenshot}",
            url,
        ]
        browser_process_sandboxed = not sys.platform.startswith("linux")
        if not browser_process_sandboxed:
            command.insert(1, "--no-sandbox")
        captured = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
        require(
            captured.returncode == 0,
            f"browser screenshot failed: {captured.stderr[:4096]!r}",
        )
        dom = subprocess.run(
            [*command[:-2], "--dump-dom", url],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
        require(
            dom.returncode == 0, f"browser DOM capture failed: {dom.stderr[:4096]!r}"
        )
        html = dom.stdout.decode("utf-8", errors="replace")
        require("<title>LaunchPad</title>" in html, "rendered title is missing")
        require(
            'role="status"' in html and 'id="task-form"' in html,
            "rendered semantics are missing",
        )
        png = screenshot.read_bytes()
        require(
            png.startswith(b"\x89PNG\r\n\x1a\n") and len(png) > 4096,
            "browser screenshot is invalid",
        )
        width, height = struct.unpack(">II", png[16:24])
        require(
            (width, height) == (1280, 720),
            "browser screenshot dimensions are not canonical",
        )
        parser = LinkParser()
        parser.feed(INDEX_HTML)
        checked = 0
        for reference in parser.references:
            parsed = urllib.parse.urlsplit(reference)
            require(
                not parsed.scheme and not parsed.netloc,
                "website contains an external reference",
            )
            if reference.startswith("#"):
                require(
                    f'id="{reference[1:]}"' in INDEX_HTML,
                    "fragment link target is missing",
                )
            else:
                with urllib.request.urlopen(
                    urllib.parse.urljoin(url, reference), timeout=10
                ) as response:
                    require(
                        response.status == 200, f"linked resource failed: {reference}"
                    )
            checked += 1
        return {
            "browser": browser_executable().name,
            "screenshot_sha256": sha256(png),
            "screenshot_bytes": len(png),
            "width": width,
            "height": height,
            "references_checked": checked,
            "browser_process_sandboxed": browser_process_sandboxed,
        }
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        shutil.rmtree(profile, ignore_errors=True)


def prepare_project(project: pathlib.Path) -> None:
    project.mkdir(parents=True, exist_ok=True)
    (project / "tests").mkdir(exist_ok=True)
    (project / "README.md").write_text("# LaunchPad\n", encoding="utf-8")
    (project / "package.json").write_text(
        json.dumps(
            {
                "name": "launchpad-installed-uat",
                "private": True,
                "type": "module",
                "scripts": {"test": "node --test tests/website.test.mjs"},
            },
            separators=(",", ":"),
        )
        + "\n",
        encoding="utf-8",
    )
    (project / "tests/website.test.mjs").write_text(WEBSITE_TEST, encoding="utf-8")


def expected_digests() -> dict[str, str]:
    return {
        "index.html": sha256(INDEX_HTML.encode()),
        "styles.css": sha256(STYLES_CSS.encode()),
        "app.js": sha256(APP_JS.encode()),
    }


def verify_files(project: pathlib.Path) -> dict[str, str]:
    actual = {
        name: sha256((project / name).read_bytes()) for name in expected_digests()
    }
    require(
        actual == expected_digests(),
        "generated website bytes do not match the reviewed fixture",
    )
    return actual


def task_audit(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    evidence_root: pathlib.Path,
    task_id: str,
    label: str,
) -> list[dict[str, Any]]:
    result = invoke_cli(
        cli, host, token_file, project, evidence_root, label, ["task", "audit", task_id]
    )
    require(result.returncode == 0, f"{label} failed: {result.stderr[:4096]!r}")
    records = decode_json(result, label)["records"]
    completions = [
        record
        for record in records
        if record.get("phase") == "completion" and record.get("result") == "succeeded"
    ]
    require(
        sum(record.get("tool") == "fs.write" for record in completions) == 3,
        "website lacks three governed writes",
    )
    require(
        any(record.get("tool") == "process.exec" for record in completions),
        "website test was not governed",
    )
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
    require(
        run_project_test(project, evidence_root, "real-project-before") != 0,
        "seeded website test unexpectedly passed",
    )
    token = token_file.read_text(encoding="utf-8").strip()
    require(bool(token), "installed token is empty")

    provider = ProviderServer()
    provider_thread = threading.Thread(target=provider.serve_forever, daemon=True)
    provider_thread.start()
    provider_id = f"installed_real_project_{uuid.uuid4().hex}"
    configured = False
    result: subprocess.CompletedProcess[bytes] | None = None
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
                    "display_name": "Installed real-project fixture",
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
        project_result = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "real-project-register",
            ["project", "add", str(project), "--json"],
        )
        require(
            project_result.returncode == 0,
            f"real project registration failed: {project_result.stderr[:4096]!r}",
        )
        project_id = decode_json(project_result, "real-project-register")["project"][
            "project_id"
        ]
        task_result = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "real-project-task",
            [
                "task",
                "create",
                "Build and verify the accessible LaunchPad website",
                "--project-id",
                project_id,
                "--json",
            ],
        )
        require(
            task_result.returncode == 0,
            f"real project task creation failed: {task_result.stderr[:4096]!r}",
        )
        task_id = decode_json(task_result, "real-project-task")["task"]["id"]
        result = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "real-project-run",
            ["task", "launch", task_id, "--json"],
        )
        require(
            result.returncode == 0, f"real project failed: {result.stderr[:4096]!r}"
        )
        value = decode_json(result, "real-project-run")
        require(
            value["task"]["state"] == "completed", "real-project task did not complete"
        )
        require(
            value["run"]["run"]["state"] == "completed",
            "real-project run did not complete",
        )
        require(
            value["run"]["evaluations"][0]["outcome"] == "passed",
            "independent evaluation did not pass",
        )
        require(
            len(value["run"]["results"]) == 2,
            "worker and evaluator results were not retained",
        )
        require(
            provider.call_count == 3,
            f"expected three provider calls, observed {provider.call_count}",
        )
        require(
            run_project_test(project, evidence_root, "real-project-after") == 0,
            "generated website regression test failed",
        )
        digests = verify_files(project)
        require(
            value["task"]["id"] == task_id, "real-project launch changed task identity"
        )
        run_id = value["run"]["run"]["id"]
        audit = task_audit(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            task_id,
            "real-project-audit",
        )
        browser = render_project(project, evidence_root, "real-project-exercise")
        replay = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "real-project-replay",
            ["run", "replay", run_id, "--json"],
        )
        require(
            replay.returncode == 0,
            f"real-project replay failed: {replay.stderr[:4096]!r}",
        )
        replay_value = decode_json(replay, "real-project-replay")
        require(
            replay_value["side_effect_free"] is True,
            "real-project replay was not side-effect free",
        )
        require(
            all(item["passed"] for item in replay_value["invariants"]),
            "real-project replay invariant failed",
        )
        require(provider.call_count == 3, "replay contacted the provider")
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
                    "browser_rendered": True,
                    "browser_process_sandboxed": browser["browser_process_sandboxed"],
                    "canonical_viewport": browser["width"] == 1280
                    and browser["height"] == 720,
                    "references_checked": browser["references_checked"],
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
    require(
        state["schema_version"] == SCHEMA_VERSION, "real-project state schema changed"
    )
    require(
        state["project_path_sha256"] == sha256(str(project).encode()),
        "real-project path changed",
    )
    require(
        verify_files(project) == state["files"],
        "real-project files changed across restart",
    )
    require(
        run_project_test(project, evidence_root, "real-project-restart-test") == 0,
        "restart regression failed",
    )
    task = invoke_cli(
        cli,
        args.host,
        token_file,
        project,
        evidence_root,
        "real-project-restart-task",
        ["task", "get", state["task_id"], "--json"],
    )
    require(
        task.returncode == 0
        and decode_json(task, "real-project-restart-task")["state"] == "completed",
        "task did not survive restart",
    )
    audit = task_audit(
        cli,
        args.host,
        token_file,
        project,
        evidence_root,
        state["task_id"],
        "real-project-restart-audit",
    )
    require(
        len(audit) == state["audit_count"], "real-project audit changed across restart"
    )
    replay = invoke_cli(
        cli,
        args.host,
        token_file,
        project,
        evidence_root,
        "real-project-restart-replay",
        ["run", "replay", state["run_id"], "--json"],
    )
    require(replay.returncode == 0, f"restart replay failed: {replay.stderr[:4096]!r}")
    replay_value = decode_json(replay, "real-project-restart-replay")
    require(
        replay_value["side_effect_free"] is True,
        "restart replay was not side-effect free",
    )
    require(
        all(item["passed"] for item in replay_value["invariants"]),
        "restart replay invariant failed",
    )
    browser = render_project(project, evidence_root, "real-project-restart")
    token = token_file.read_text(encoding="utf-8").strip().encode()
    require(
        token not in task.stdout and token not in task.stderr,
        "restart output exposed its token",
    )
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
                "browser_rendered": True,
                "browser_process_sandboxed": browser["browser_process_sandboxed"],
                "canonical_viewport": browser["width"] == 1280
                and browser["height"] == 720,
                "references_checked": browser["references_checked"],
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
