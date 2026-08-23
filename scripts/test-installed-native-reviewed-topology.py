#!/usr/bin/env python3
"""Prepare and verify a real native reviewed-topology desktop journey."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import time
import urllib.error
import urllib.request
import uuid
from typing import Any


SCHEMA_VERSION = 1
PROJECT_DIRECTORY = "native-reviewed-topology-project"
PROJECT_NAME = "native-reviewed-topology-project"
PLAN_REASON = "Run the installed native reviewed project tests"
PLAN_DISPLAY_NAME = f"test · {PROJECT_NAME}"
MARKER = "execution-count.txt"

PACKAGE_JSON = {
    "name": "installed-native-reviewed-topology-fixture",
    "private": True,
    "type": "module",
    "scripts": {
        "test": "node -e \"require('fs').appendFileSync('execution-count.txt','x')\""
    },
}

COMPOSE_YAML = """services:
  database:
    image: postgres:17
  api:
    image: installed-native-topology-api
    depends_on:
      - database
  web:
    image: installed-native-topology-web
    depends_on:
      - api
"""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def write_atomic_json(path: pathlib.Path, value: object) -> None:
    require(not path.exists(), f"refusing to replace {path.name}")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{uuid.uuid4().hex}.tmp")
    with temporary.open("x", encoding="utf-8", newline="\n") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")
    try:
        os.link(temporary, path)
        temporary.unlink()
    except Exception:
        temporary.unlink(missing_ok=True)
        raise


def read_token(path: pathlib.Path) -> str:
    token = path.read_text(encoding="utf-8").strip()
    require(bool(token), "installed loopback token is empty")
    return token


def api_request(
    host: str,
    token: str,
    method: str,
    route: str,
    body: object | None = None,
) -> tuple[int, bytes]:
    encoded = None if body is None else json.dumps(body, separators=(",", ":")).encode()
    request = urllib.request.Request(
        f"{host.rstrip('/')}{route}",
        data=encoded,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": "application/json",
        },
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read(4096)


def api_json(
    host: str,
    token: str,
    method: str,
    route: str,
    body: object | None = None,
) -> Any:
    status, payload = api_request(host, token, method, route, body)
    require(
        200 <= status < 300,
        f"{method} {route} returned HTTP {status}: {payload[:2048]!r}",
    )
    return None if not payload else json.loads(payload)


def put_settings(
    host: str,
    token: str,
    current_revision: int,
    settings: dict[str, Any],
    reason: str,
) -> dict[str, Any]:
    return api_json(
        host,
        token,
        "PUT",
        "/api/v1/settings",
        {
            "expected_revision": current_revision,
            "advanced_mode": settings["advanced_mode"],
            "autonomy": settings["autonomy"],
            "shadow_experiments": settings["shadow_experiments"],
            "reason": reason,
        },
    )


def prepare(args: argparse.Namespace) -> None:
    workspace = pathlib.Path(args.workspace).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    state_path = pathlib.Path(args.state).resolve()
    output_path = pathlib.Path(args.output).resolve()
    token = read_token(token_file)
    project = workspace / PROJECT_DIRECTORY
    require(
        not project.exists(), f"refusing to replace existing fixture {project.name}"
    )
    project.mkdir(parents=True)
    project.joinpath("package.json").write_text(
        json.dumps(PACKAGE_JSON, separators=(",", ":")) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    project.joinpath("compose.yaml").write_text(
        COMPOSE_YAML, encoding="utf-8", newline="\n"
    )

    original = api_json(args.host, token, "GET", "/api/v1/settings")
    try:
        project_id = str(uuid.uuid4())
        discovered = api_json(
            args.host,
            token,
            "POST",
            "/api/v1/projects",
            {"root": str(project), "project_id": project_id},
        )
        require(
            discovered["project"]["project_id"] == project_id,
            "native project identity mismatch",
        )
        commands = [
            command
            for command in discovered["project"]["commands"]
            if command["name"] == "test" and command["command"] == "npm run test"
        ]
        require(len(commands) == 1, "native project omitted its exact test command")
        topology = api_json(
            args.host, token, "GET", f"/api/v1/projects/{project_id}/topology-plan"
        )
        labels = [
            node["label"] for stage in topology["stages"] for node in stage["nodes"]
        ]
        require(topology["schema_version"] == 2, "native topology schema mismatch")
        require(topology["status"] == "ready", "native topology is not ready")
        require(len(topology["stages"]) == 3, "native topology is not three stages")
        require(
            {"database", "api", "web"}.issubset(labels),
            "native topology omitted database/api/web",
        )

        state = {
            "schema_version": SCHEMA_VERSION,
            "project_directory": PROJECT_DIRECTORY,
            "project_id": project_id,
            "project_snapshot_id": topology["project_snapshot_id"],
            "topology_sha256": topology["sha256"],
            "original_settings": {
                "advanced_mode": original["advanced_mode"],
                "autonomy": original["autonomy"],
                "shadow_experiments": original["shadow_experiments"],
            },
        }
        report = {
            "schema_version": SCHEMA_VERSION,
            "phase": "prepare",
            "project_name": PROJECT_NAME,
            "topology_schema_version": topology["schema_version"],
            "topology_sha256": topology["sha256"],
            "topology_stage_count": len(topology["stages"]),
            "topology_labels": labels,
            "discovered_command": commands[0]["command"],
            "guided_autonomy_pending_native_ui": True,
            "native_ui_mutation_pending": True,
            "private_path_persisted": False,
        }
        encoded = json.dumps(report, sort_keys=True)
        require(
            str(workspace) not in encoded and token not in encoded,
            "private data entered native topology preparation report",
        )
        write_atomic_json(state_path, state)
        write_atomic_json(output_path, report)
    except Exception:
        raise


def verify(args: argparse.Namespace) -> None:
    workspace = pathlib.Path(args.workspace).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    state_path = pathlib.Path(args.state).resolve(strict=True)
    output_path = pathlib.Path(args.output).resolve()
    token = read_token(token_file)
    state = json.loads(state_path.read_text(encoding="utf-8"))
    require(state["schema_version"] == SCHEMA_VERSION, "native state schema mismatch")
    project = workspace / state["project_directory"]
    require(
        project.resolve(strict=True).parent == workspace,
        "native project escaped its workspace",
    )

    plan: dict[str, Any] | None = None
    pending: list[dict[str, Any]] = []
    restored_settings: dict[str, Any] | None = None
    try:
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            plans = api_json(
                args.host,
                token,
                "GET",
                f"/api/v1/projects/{state['project_id']}/reviewed-topology-plans",
            )["plans"]
            matches = [
                candidate for candidate in plans if candidate["reason"] == PLAN_REASON
            ]
            if len(matches) == 1 and matches[0]["state"] == "succeeded":
                plan = matches[0]
                break
            time.sleep(0.1)
        require(plan is not None, "native reviewed topology plan did not succeed")
        require(plan["display_name"] == PLAN_DISPLAY_NAME, "native plan name mismatch")
        require(
            plan["project_snapshot_id"] == state["project_snapshot_id"],
            "native plan snapshot mismatch",
        )
        require(
            plan["topology_sha256"] == state["topology_sha256"],
            "native plan topology mismatch",
        )
        require(len(plan["steps"]) == 1, "native plan has the wrong step count")
        step = plan["steps"][0]
        require(
            step["command"] == "npm run test"
            and step["state"] == "succeeded"
            and step["exit_code"] == 0,
            "native reviewed command did not succeed",
        )
        require(
            len(step["log_artifact_sha256"]) >= 1,
            "native reviewed command retained no log artifact",
        )
        require(
            project.joinpath(MARKER).read_text(encoding="utf-8") == "x",
            "native reviewed command did not execute exactly once",
        )
        pending = api_json(args.host, token, "GET", "/api/v1/approvals?state=pending")[
            "approvals"
        ]
        require(len(pending) == 0, "native reviewed approval remains pending")
    finally:
        current = api_json(args.host, token, "GET", "/api/v1/settings")
        restored_settings = put_settings(
            args.host,
            token,
            current["revision"],
            state["original_settings"],
            "Restore settings after native reviewed topology UAT",
        )

    require(plan is not None, "native reviewed topology plan is missing")
    for field in ["advanced_mode", "autonomy", "shadow_experiments"]:
        require(
            restored_settings[field] == state["original_settings"][field],
            f"native UAT did not restore {field}",
        )
    step = plan["steps"][0]
    report = {
        "schema_version": SCHEMA_VERSION,
        "phase": "verify",
        "project_name": PROJECT_NAME,
        "topology_sha256": state["topology_sha256"],
        "plan_state": plan["state"],
        "plan_definition_sha256": plan["definition_hash"],
        "step_log_artifact_sha256": step["log_artifact_sha256"],
        "command_executions": 1,
        "approval_resolved": True,
        "settings_restored": True,
        "private_path_persisted": False,
    }
    encoded = json.dumps(report, sort_keys=True)
    require(
        str(workspace) not in encoded and token not in encoded,
        "private data entered native topology verification report",
    )
    write_atomic_json(output_path, report)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("prepare", "verify"), required=True)
    parser.add_argument("--host", required=True)
    parser.add_argument("--token-file", required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--state", required=True)
    parser.add_argument("--output", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.mode == "prepare":
        prepare(args)
    else:
        verify(args)
    print(
        json.dumps(
            {"schema_version": SCHEMA_VERSION, "mode": args.mode, "passed": True}
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
