#!/usr/bin/env python3
"""Qualify installed project topology review/execution across REST, CLI, and MCP."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import urllib.error
import urllib.request
import uuid
from typing import Any


SCHEMA_VERSION = 1
MCP_VERSION = "2026-07-28"
PRIMARY_DIRECTORY = "reviewed-topology-project"
STALE_DIRECTORY = "reviewed-topology-stale-project"
MARKER = "execution-count.txt"

PACKAGE_JSON = {
    "name": "installed-reviewed-topology-fixture",
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
    image: installed-topology-api
    depends_on:
      - database
  web:
    image: installed-topology-web
    depends_on:
      - api
"""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


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


def read_token(token_file: pathlib.Path) -> str:
    token = token_file.read_text(encoding="utf-8").strip()
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


def expect_api_error(
    host: str,
    token: str,
    method: str,
    route: str,
    expected_status: int,
    body: object | None = None,
) -> bytes:
    status, payload = api_request(host, token, method, route, body)
    require(
        status == expected_status,
        f"{method} {route} returned HTTP {status}, expected {expected_status}",
    )
    require(token.encode() not in payload, "loopback token leaked into API error")
    return payload


def mcp_tool(
    host: str,
    token: str,
    request_id: int,
    name: str,
    arguments: dict[str, Any],
) -> dict[str, Any]:
    payload = {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments,
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": MCP_VERSION,
                "io.modelcontextprotocol/clientInfo": {
                    "name": "mcg-installed-reviewed-topology-uat",
                    "version": "1.0.0",
                },
                "io.modelcontextprotocol/clientCapabilities": {},
            },
        },
    }
    request = urllib.request.Request(
        f"{host.rstrip('/')}/mcp",
        data=json.dumps(payload, separators=(",", ":")).encode(),
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
            "MCP-Protocol-Version": MCP_VERSION,
            "Mcp-Method": "tools/call",
            "Mcp-Name": name,
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=120) as response:
        value = json.load(response)
    require("error" not in value, f"MCP transport failed: {value}")
    return value["result"]


def cli_json(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    arguments: list[str],
) -> tuple[Any, str]:
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
    require(
        result.returncode == 0,
        f"installed CLI failed ({result.returncode}): {result.stderr[:2048]!r}",
    )
    try:
        value = json.loads(result.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AssertionError(
            f"installed CLI did not return JSON: {result.stdout[:2048]!r}"
        ) from error
    return value, sha256_bytes(result.stdout)


def prepare_project(path: pathlib.Path) -> None:
    require(not path.exists(), f"refusing to replace existing fixture {path.name}")
    path.mkdir(parents=True)
    path.joinpath("package.json").write_text(
        json.dumps(PACKAGE_JSON, separators=(",", ":")) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    path.joinpath("compose.yaml").write_text(
        COMPOSE_YAML, encoding="utf-8", newline="\n"
    )


def command_and_topology(
    host: str,
    token: str,
    project_id: str,
    discovered: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    commands = [
        command
        for command in discovered["project"]["commands"]
        if command["name"] == "test" and command["command"] == "npm run test"
    ]
    require(
        len(commands) == 1,
        "project discovery did not expose the exact npm test command",
    )
    topology = api_json(
        host, token, "GET", f"/api/v1/projects/{project_id}/topology-plan"
    )
    require(topology["schema_version"] == 2, "topology schema mismatch")
    require(topology["status"] == "ready", "fixture topology is not ready")
    require(
        topology["project_snapshot_id"] == discovered["project"]["id"],
        "topology snapshot mismatch",
    )
    require(len(topology["sha256"]) == 64, "topology digest is invalid")
    require(
        len(topology["stages"]) >= 3,
        "explicit database/api/web dependencies did not form three stages",
    )
    labels = [node["label"] for stage in topology["stages"] for node in stage["nodes"]]
    require(
        {"database", "api", "web"}.issubset(labels),
        "topology omitted named Compose services",
    )
    relationships = discovered["project"]["world"]["relationships"]
    require(
        sum(
            1
            for relationship in relationships
            if relationship["relation"] == "depends_on"
        )
        >= 2,
        "world model omitted explicit dependency edges",
    )
    return commands[0], topology


def create_request(
    topology: dict[str, Any], command: dict[str, Any], name: str
) -> dict[str, Any]:
    return {
        "id": str(uuid.uuid4()),
        "project_snapshot_id": topology["project_snapshot_id"],
        "topology_sha256": topology["sha256"],
        "display_name": name,
        "reason": "Qualify separately reviewed installed project execution",
        "steps": [
            {
                "id": str(uuid.uuid4()),
                "stage_index": topology["stages"][-1]["index"],
                "name": "Run discovered project tests",
                "command_name": command["name"],
                "command": command["command"],
                "timeout_seconds": 120,
            }
        ],
    }


def set_settings(
    host: str, token: str, current: dict[str, Any], autonomy: str, reason: str
) -> dict[str, Any]:
    return api_json(
        host,
        token,
        "PUT",
        "/api/v1/settings",
        {
            "expected_revision": current["revision"],
            "advanced_mode": current["advanced_mode"],
            "autonomy": autonomy,
            "shadow_experiments": current["shadow_experiments"],
            "reason": reason,
        },
    )


def plan_digest(plan: dict[str, Any]) -> str:
    bounded = {
        "definition_hash": plan["definition_hash"],
        "project_snapshot_id": plan["project_snapshot_id"],
        "topology_sha256": plan["topology_sha256"],
        "state": plan["state"],
        "steps": [
            {
                "id": step["id"],
                "stage_index": step["stage_index"],
                "command": step["command"],
                "executable": step["executable"],
                "arguments": step["arguments"],
                "state": step["state"],
                "exit_code": step["exit_code"],
                "log_artifact_sha256": step["log_artifact_sha256"],
            }
            for step in plan["steps"]
        ],
    }
    return sha256_bytes(
        json.dumps(bounded, sort_keys=True, separators=(",", ":")).encode()
    )


def exercise(args: argparse.Namespace) -> None:
    workspace = pathlib.Path(args.workspace).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    cli = pathlib.Path(args.cli).resolve(strict=True)
    state_path = pathlib.Path(args.state).resolve()
    output_path = pathlib.Path(args.output).resolve()
    token = read_token(token_file)
    primary = workspace / PRIMARY_DIRECTORY
    stale = workspace / STALE_DIRECTORY
    prepare_project(primary)
    prepare_project(stale)

    original_settings = api_json(args.host, token, "GET", "/api/v1/settings")
    restored = False
    cli_hashes: list[str] = []
    try:
        guided = set_settings(
            args.host,
            token,
            original_settings,
            "guided",
            "Qualify installed reviewed topology approval",
        )
        require(guided["autonomy"] == "guided", "guided settings did not apply")

        project_id = str(uuid.uuid4())
        discovered = api_json(
            args.host,
            token,
            "POST",
            "/api/v1/projects",
            {"root": str(primary), "project_id": project_id},
        )
        require(
            discovered["project"]["project_id"] == project_id,
            "project identity mismatch",
        )
        command, topology = command_and_topology(
            args.host, token, project_id, discovered
        )
        cli_topology, cli_hash = cli_json(
            cli, args.host, token_file, primary, ["project", "plan", project_id]
        )
        cli_hashes.append(cli_hash)
        require(
            cli_topology["sha256"] == topology["sha256"],
            "CLI/REST topology digest mismatch",
        )

        request = create_request(topology, command, "Installed reviewed topology test")
        base = f"/api/v1/projects/{project_id}/reviewed-topology-plans"
        created = api_json(args.host, token, "POST", base, request)
        require(
            created["plan"]["state"] == "reviewed" and created["replayed"] is False,
            "review was not created",
        )
        require(
            not primary.joinpath(MARKER).exists(),
            "review unexpectedly executed the project command",
        )
        replay = api_json(args.host, token, "POST", base, request)
        require(
            replay["replayed"] is True and replay["plan"]["state"] == "reviewed",
            "review replay mutated state",
        )

        mcp_query = mcp_tool(
            args.host,
            token,
            1,
            "mcg_reviewed_topology_plans",
            {"project_id": project_id, "plan_id": request["id"]},
        )
        require(
            mcp_query.get("isError") is not True,
            "MCP could not query the reviewed plan",
        )
        require(
            mcp_query["structuredContent"]["plan"]["state"] == "reviewed",
            "MCP review state mismatch",
        )

        waiting, cli_hash = cli_json(
            cli,
            args.host,
            token_file,
            primary,
            ["project", "reviewed", project_id, "run", request["id"]],
        )
        cli_hashes.append(cli_hash)
        require(
            waiting["plan"]["state"] == "waiting",
            "guided execution did not wait for approval",
        )
        approval_id = waiting.get("approval_id")
        require(
            isinstance(approval_id, str),
            "guided execution did not return an approval identity",
        )
        pending = api_json(args.host, token, "GET", "/api/v1/approvals?state=pending")[
            "approvals"
        ]
        matches = [approval for approval in pending if approval["id"] == approval_id]
        require(
            len(matches) == 1, "the exact reviewed execution approval is not pending"
        )
        require(
            not primary.joinpath(MARKER).exists(),
            "waiting execution changed the project",
        )
        resolved = api_json(
            args.host,
            token,
            "POST",
            f"/api/v1/approvals/{approval_id}/resolve",
            {
                "state": "approved",
                "reason": "Run the exact snapshot-pinned test command",
            },
        )
        require(
            resolved["state"] == "approved",
            "reviewed execution approval did not resolve",
        )

        executed = mcp_tool(
            args.host,
            token,
            2,
            "mcg_reviewed_topology_plan_mutate",
            {"action": "run", "project_id": project_id, "plan_id": request["id"]},
        )
        require(executed.get("isError") is not True, "MCP reviewed execution failed")
        mutation = executed["structuredContent"]
        plan = mutation["plan"]
        require(
            plan["state"] == "succeeded" and mutation["replayed"] is False,
            "reviewed execution did not succeed",
        )
        require(
            len(mutation["outcomes"]) == 1,
            "reviewed execution did not retain one governed outcome",
        )
        step = plan["steps"][0]
        require(
            step["state"] == "succeeded" and step["exit_code"] == 0,
            "reviewed step did not succeed",
        )
        require(
            len(step["log_artifact_sha256"]) >= 1,
            "reviewed step has no content-addressed log evidence",
        )
        require(
            primary.joinpath(MARKER).read_text(encoding="utf-8") == "x",
            "reviewed command did not execute exactly once",
        )

        terminal_replay, cli_hash = cli_json(
            cli,
            args.host,
            token_file,
            primary,
            ["project", "reviewed", project_id, "run", request["id"]],
        )
        cli_hashes.append(cli_hash)
        require(
            terminal_replay["replayed"] is True,
            "terminal CLI replay was not observation-only",
        )
        require(
            primary.joinpath(MARKER).read_text(encoding="utf-8") == "x",
            "terminal replay executed the command again",
        )

        queried, cli_hash = cli_json(
            cli,
            args.host,
            token_file,
            primary,
            ["project", "reviewed", project_id, "get", request["id"]],
        )
        cli_hashes.append(cli_hash)
        require(
            plan_digest(queried) == plan_digest(plan),
            "CLI/REST/MCP plan evidence diverged",
        )

        stale_project_id = str(uuid.uuid4())
        stale_discovered = api_json(
            args.host,
            token,
            "POST",
            "/api/v1/projects",
            {"root": str(stale), "project_id": stale_project_id},
        )
        stale_command, stale_topology = command_and_topology(
            args.host, token, stale_project_id, stale_discovered
        )
        stale_request = create_request(
            stale_topology, stale_command, "Stale reviewed topology test"
        )
        stale_base = f"/api/v1/projects/{stale_project_id}/reviewed-topology-plans"
        stale_created = api_json(args.host, token, "POST", stale_base, stale_request)
        require(
            stale_created["plan"]["state"] == "reviewed",
            "stale fixture review was not created",
        )
        stale.joinpath("compose.yaml").write_text(
            COMPOSE_YAML
            + "  worker:\n    image: installed-topology-worker\n    depends_on:\n      - database\n",
            encoding="utf-8",
            newline="\n",
        )
        expect_api_error(
            args.host,
            token,
            "POST",
            f"{stale_base}/{stale_request['id']}/run",
            409,
        )
        require(
            not stale.joinpath(MARKER).exists(), "stale authority executed a command"
        )

        current_settings = api_json(args.host, token, "GET", "/api/v1/settings")
        restored_settings = set_settings(
            args.host,
            token,
            current_settings,
            original_settings["autonomy"],
            "Restore settings after reviewed topology qualification",
        )
        restored = True
        require(
            restored_settings["autonomy"] == original_settings["autonomy"],
            "autonomy was not restored",
        )
        require(
            restored_settings["advanced_mode"] == original_settings["advanced_mode"],
            "advanced mode changed",
        )
        require(
            restored_settings["shadow_experiments"]
            == original_settings["shadow_experiments"],
            "shadow setting changed",
        )

        state = {
            "schema_version": SCHEMA_VERSION,
            "primary_directory": PRIMARY_DIRECTORY,
            "stale_directory": STALE_DIRECTORY,
            "project_id": project_id,
            "project_snapshot_id": topology["project_snapshot_id"],
            "topology_sha256": topology["sha256"],
            "plan_id": request["id"],
            "plan_definition_hash": plan["definition_hash"],
            "plan_digest": plan_digest(plan),
            "step_id": step["id"],
            "log_artifact_sha256": step["log_artifact_sha256"],
            "approval_id": approval_id,
            "stale_project_id": stale_project_id,
            "stale_plan_id": stale_request["id"],
            "original_settings": {
                "advanced_mode": original_settings["advanced_mode"],
                "autonomy": original_settings["autonomy"],
                "shadow_experiments": original_settings["shadow_experiments"],
                "revision": original_settings["revision"],
            },
            "restored_settings_revision": restored_settings["revision"],
        }
        report = {
            "schema_version": SCHEMA_VERSION,
            "phase": "exercise",
            "topology_schema_version": topology["schema_version"],
            "topology_sha256": topology["sha256"],
            "topology_stage_count": len(topology["stages"]),
            "world_dependency_count": sum(
                1
                for relationship in discovered["project"]["world"]["relationships"]
                if relationship["relation"] == "depends_on"
            ),
            "review_replay_observation_only": replay["replayed"],
            "approval_required_and_resolved": resolved["state"] == "approved",
            "plan_digest": state["plan_digest"],
            "step_log_artifact_sha256": step["log_artifact_sha256"],
            "command_executions": 1,
            "terminal_replay_observation_only": terminal_replay["replayed"],
            "stale_authority_rejected": True,
            "settings_restored": True,
            "cli_transcript_sha256": sorted(cli_hashes),
            "rest_query_mcp_agree": True,
            "private_path_persisted": False,
        }
        encoded = json.dumps(report, sort_keys=True)
        require(
            str(workspace) not in encoded and token not in encoded,
            "private data entered topology report",
        )
        write_atomic_json(state_path, state)
        write_atomic_json(output_path, report)
    finally:
        if not restored:
            try:
                current = api_json(args.host, token, "GET", "/api/v1/settings")
                if (
                    current["advanced_mode"] != original_settings["advanced_mode"]
                    or current["autonomy"] != original_settings["autonomy"]
                    or current["shadow_experiments"]
                    != original_settings["shadow_experiments"]
                ):
                    set_settings(
                        args.host,
                        token,
                        current,
                        original_settings["autonomy"],
                        "Restore settings after interrupted topology qualification",
                    )
            except Exception:  # noqa: BLE001 - preserve the primary qualification failure
                pass


def verify(args: argparse.Namespace) -> None:
    workspace = pathlib.Path(args.workspace).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    cli = pathlib.Path(args.cli).resolve(strict=True)
    state_path = pathlib.Path(args.state).resolve(strict=True)
    output_path = pathlib.Path(args.output).resolve()
    token = read_token(token_file)
    state = json.loads(state_path.read_text(encoding="utf-8"))
    require(state["schema_version"] == SCHEMA_VERSION, "topology state schema mismatch")
    primary = workspace / state["primary_directory"]
    stale = workspace / state["stale_directory"]
    require(
        primary.resolve(strict=True).parent == workspace,
        "primary fixture escaped workspace",
    )
    require(
        stale.resolve(strict=True).parent == workspace,
        "stale fixture escaped workspace",
    )
    require(
        primary.joinpath(MARKER).read_text(encoding="utf-8") == "x",
        "execution marker changed across restart",
    )
    require(
        not stale.joinpath(MARKER).exists(), "stale fixture executed across restart"
    )

    settings = api_json(args.host, token, "GET", "/api/v1/settings")
    original = state["original_settings"]
    require(
        settings["revision"] == state["restored_settings_revision"],
        "settings revision changed across restart",
    )
    for field in ["advanced_mode", "autonomy", "shadow_experiments"]:
        require(
            settings[field] == original[field],
            f"{field} was not restored across restart",
        )
    history = api_json(args.host, token, "GET", "/api/v1/settings/history")["revisions"]
    reasons = {entry["reason"] for entry in history}
    require(
        "Qualify installed reviewed topology approval" in reasons,
        "guided settings revision is absent",
    )
    require(
        "Restore settings after reviewed topology qualification" in reasons,
        "restored settings revision is absent",
    )

    project = api_json(
        args.host, token, "GET", f"/api/v1/projects/{state['project_id']}"
    )
    require(
        project["id"] == state["project_snapshot_id"],
        "project snapshot changed across restart",
    )
    topology = api_json(
        args.host, token, "GET", f"/api/v1/projects/{state['project_id']}/topology-plan"
    )
    require(
        topology["sha256"] == state["topology_sha256"],
        "topology digest changed across restart",
    )
    base = f"/api/v1/projects/{state['project_id']}/reviewed-topology-plans"
    plan = api_json(args.host, token, "GET", f"{base}/{state['plan_id']}")
    require(plan["state"] == "succeeded", "reviewed plan did not survive restart")
    require(
        plan["definition_hash"] == state["plan_definition_hash"],
        "plan definition changed across restart",
    )
    require(
        plan_digest(plan) == state["plan_digest"],
        "plan evidence changed across restart",
    )
    require(
        plan["steps"][0]["log_artifact_sha256"] == state["log_artifact_sha256"],
        "log evidence changed across restart",
    )

    cli_topology, cli_topology_hash = cli_json(
        cli, args.host, token_file, primary, ["project", "plan", state["project_id"]]
    )
    require(
        cli_topology["sha256"] == state["topology_sha256"],
        "CLI topology changed across restart",
    )
    cli_plan, cli_plan_hash = cli_json(
        cli,
        args.host,
        token_file,
        primary,
        ["project", "reviewed", state["project_id"], "get", state["plan_id"]],
    )
    require(
        plan_digest(cli_plan) == state["plan_digest"], "CLI plan changed across restart"
    )

    mcp_query = mcp_tool(
        args.host,
        token,
        20,
        "mcg_reviewed_topology_plans",
        {"project_id": state["project_id"], "plan_id": state["plan_id"]},
    )
    require(mcp_query.get("isError") is not True, "MCP plan query failed after restart")
    require(
        plan_digest(mcp_query["structuredContent"]["plan"]) == state["plan_digest"],
        "MCP plan changed across restart",
    )
    replay = mcp_tool(
        args.host,
        token,
        21,
        "mcg_reviewed_topology_plan_mutate",
        {
            "action": "run",
            "project_id": state["project_id"],
            "plan_id": state["plan_id"],
        },
    )
    require(replay.get("isError") is not True, "terminal MCP replay failed")
    require(
        replay["structuredContent"]["replayed"] is True,
        "terminal MCP replay was not observation-only",
    )
    require(
        primary.joinpath(MARKER).read_text(encoding="utf-8") == "x",
        "restart replay executed the command again",
    )

    stale_result = mcp_tool(
        args.host,
        token,
        22,
        "mcg_reviewed_topology_plan_mutate",
        {
            "action": "run",
            "project_id": state["stale_project_id"],
            "plan_id": state["stale_plan_id"],
        },
    )
    require(
        stale_result.get("isError") is True,
        "stale reviewed authority became executable after restart",
    )
    require(
        "conflict" in json.dumps(stale_result).lower(),
        "MCP stale-authority result is not a conflict",
    )
    pending = api_json(args.host, token, "GET", "/api/v1/approvals?state=pending")[
        "approvals"
    ]
    require(
        all(approval["id"] != state["approval_id"] for approval in pending),
        "resolved topology approval became pending",
    )

    report = {
        "schema_version": SCHEMA_VERSION,
        "phase": "verify",
        "daemon_restart_verified": True,
        "topology_sha256": state["topology_sha256"],
        "plan_digest": state["plan_digest"],
        "step_log_artifact_sha256": state["log_artifact_sha256"],
        "command_executions": 1,
        "terminal_replay_observation_only": replay["structuredContent"]["replayed"],
        "stale_authority_rejected": stale_result.get("isError") is True,
        "settings_restored": True,
        "cli_transcript_sha256": sorted([cli_topology_hash, cli_plan_hash]),
        "rest_cli_mcp_agree": True,
        "private_path_persisted": False,
    }
    encoded = json.dumps(report, sort_keys=True)
    require(
        str(workspace) not in encoded and token not in encoded,
        "private data entered restart report",
    )
    write_atomic_json(output_path, report)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("exercise", "verify"), required=True)
    parser.add_argument("--host", required=True)
    parser.add_argument("--token-file", required=True)
    parser.add_argument("--cli", required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--state", required=True)
    parser.add_argument("--output", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.mode == "exercise":
        exercise(args)
    else:
        verify(args)
    print(
        json.dumps(
            {"schema_version": SCHEMA_VERSION, "mode": args.mode, "passed": True}
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:  # noqa: BLE001 - qualification must fail closed
        print(
            f"installed reviewed-topology qualification failed: {error}",
            file=sys.stderr,
        )
        raise
