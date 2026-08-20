#!/usr/bin/env python3
"""Exercise and verify the installed MCG measured-experiment lifecycle."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import tempfile
import urllib.error
import urllib.request
import uuid
from datetime import datetime, timezone
from typing import Any


MCP_VERSION = "2026-07-28"


def now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def identifier() -> str:
    return str(uuid.uuid4())


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


class InstalledExperimentVerifier:
    def __init__(self, host: str, token_file: pathlib.Path, cli: pathlib.Path) -> None:
        self.host = host.rstrip("/")
        self.token_file = token_file.resolve()
        self.cli = cli.resolve()
        self.token = self.token_file.read_text(encoding="utf-8").strip()
        require(bool(self.token), "the installed daemon token is empty")
        require(self.cli.is_file(), "the installed CLI does not exist")

    def request(self, method: str, path: str, body: Any | None = None) -> Any:
        data = None if body is None else json.dumps(body, separators=(",", ":")).encode()
        request = urllib.request.Request(
            f"{self.host}{path}",
            data=data,
            headers={
                "Authorization": f"Bearer {self.token}",
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
            method=method,
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            detail = error.read(4096).decode("utf-8", errors="replace")
            raise RuntimeError(f"{method} {path} failed with HTTP {error.code}: {detail}") from error

    def cli_json(self, *arguments: str) -> Any:
        command = [
            os.fspath(self.cli),
            "--host",
            self.host,
            "--token-file",
            os.fspath(self.token_file),
            *arguments,
        ]
        result = subprocess.run(command, capture_output=True, text=True, timeout=60, check=False)
        require(
            result.returncode == 0,
            f"installed CLI failed ({result.returncode}): {result.stderr[:4096]}",
        )
        try:
            return json.loads(result.stdout)
        except json.JSONDecodeError as error:
            raise RuntimeError("installed CLI did not return one JSON document") from error

    def mcp_tool(self, request_id: int, name: str, arguments: dict[str, Any]) -> Any:
        params = {
            "name": name,
            "arguments": arguments,
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": MCP_VERSION,
                "io.modelcontextprotocol/clientInfo": {
                    "name": "mcg-installed-experiment-uat",
                    "version": "1.0.0",
                },
                "io.modelcontextprotocol/clientCapabilities": {},
            },
        }
        request = urllib.request.Request(
            f"{self.host}/mcp",
            data=json.dumps(
                {"jsonrpc": "2.0", "id": request_id, "method": "tools/call", "params": params},
                separators=(",", ":"),
            ).encode(),
            headers={
                "Authorization": f"Bearer {self.token}",
                "Content-Type": "application/json",
                "Accept": "application/json, text/event-stream",
                "MCP-Protocol-Version": MCP_VERSION,
                "Mcp-Method": "tools/call",
                "Mcp-Name": name,
            },
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            value = json.load(response)
        require("error" not in value, "MCP returned a JSON-RPC error")
        require(value.get("result", {}).get("isError") is not True, "MCP tool returned an error")
        return value.get("result", {}).get("structuredContent", {})


def write_fixture(directory: pathlib.Path, name: str, value: Any) -> pathlib.Path:
    path = directory / name
    path.write_text(json.dumps(value, separators=(",", ":")), encoding="utf-8")
    return path


def arm(revision_id: str, evidence: str) -> dict[str, Any]:
    return {
        "organization_revision_id": revision_id,
        "run_id": identifier(),
        "evaluation_id": identifier(),
        "outcome": "passed",
        "confidence": {"basis_points": 9000, "sample_size": 10},
        "metrics": {
            "agent_instances": 1,
            "model_calls": 0,
            "tool_calls": 1,
            "useful_work_seconds": 1,
            "coordination_seconds": 0,
            "tokens": 0,
            "cost_usd": 0.0,
        },
        "evidence_sha256": [evidence],
    }


def exercise(verifier: InstalledExperimentVerifier, state_path: pathlib.Path) -> dict[str, Any]:
    created = verifier.request("POST", "/api/v1/organizations", {"name": "Installed benchmark UAT"})
    baseline = created["organization"]
    task = verifier.request(
        "POST", "/api/v1/tasks", {"goal": "Retain installed experiment source evidence"}
    )["task"]
    candidate = dict(baseline)
    candidate["revision_id"] = identifier()
    candidate["revision"] = baseline["revision"] + 1
    candidate["parent_revision_id"] = baseline["revision_id"]
    candidate["name"] = "Installed benchmark UAT candidate"
    candidate["created_at"] = now()

    observation = {
        "schema_version": 1,
        "id": identifier(),
        "loop_kind": "medium",
        "signal": "repeated_failure",
        "severity_basis_points": 7000,
        "recommended_intervention": "reframe",
        "task_id": task["id"],
        "run_id": None,
        "organization_revision_id": baseline["revision_id"],
        "summary": "Installed fixture found a bounded repeated failure",
        "evidence": [
            {
                "method": "installed_package_uat",
                "artifact_sha256": "a" * 64,
                "event_sequence": None,
                "detail": "Versioned cross-interface experiment fixture",
            }
        ],
        "observed_at": now(),
    }
    experiment_time = now()
    experiment = {
        "schema_version": 1,
        "id": identifier(),
        "organization_id": baseline["organization_id"],
        "baseline_revision_id": baseline["revision_id"],
        "candidate_revision_id": candidate["revision_id"],
        "mode": "shadow",
        "state": "draft",
        "hypothesis": "The candidate reduces repeated failures",
        "task_classes": ["coding/build"],
        "required_benchmarks": 1,
        "maximum_regressions": 0,
        "minimum_confidence": {"basis_points": 8000, "sample_size": 10},
        "observation_ids": [observation["id"]],
        "created_at": experiment_time,
        "updated_at": experiment_time,
    }
    receipt = {
        "schema_version": 1,
        "experiment_id": experiment["id"],
        "source_task_id": task["id"],
        "candidate_run_id": identifier(),
        "authority": "evaluation_only",
        "output_artifact_sha256": "b" * 64,
        "completed_at": now(),
    }
    benchmark = {
        "schema_version": 1,
        "id": identifier(),
        "experiment_id": experiment["id"],
        "scenario_key": "coding/build",
        "baseline": arm(baseline["revision_id"], "c" * 64),
        "candidate": arm(candidate["revision_id"], "d" * 64),
        "regressions": [],
        "measured_at": now(),
    }
    decision = {
        "schema_version": 1,
        "id": identifier(),
        "experiment_id": experiment["id"],
        "kind": "promote",
        "from_revision_id": baseline["revision_id"],
        "to_revision_id": candidate["revision_id"],
        "benchmark_ids": [benchmark["id"]],
        "trigger_evaluation_ids": [],
        "prospective_analysis": {
            "expected_benefits": ["fewer repeated failures"],
            "expected_risks": ["fixture regression"],
            "historical_evidence": [observation["id"]],
        },
        "reason": "The installed paired benchmark passed with bounded evidence",
        "decided_at": now(),
    }

    with tempfile.TemporaryDirectory(prefix="mcg-installed-experiment-") as temporary:
        fixture = pathlib.Path(temporary)
        candidate_result = verifier.cli_json(
            "experiment", "candidate", os.fspath(write_fixture(fixture, "candidate.json", candidate))
        )
        observation_result = verifier.cli_json(
            "experiment", "observe", os.fspath(write_fixture(fixture, "observation.json", observation))
        )
        experiment_result = verifier.cli_json(
            "experiment", "create", os.fspath(write_fixture(fixture, "experiment.json", experiment))
        )
        require(candidate_result.get("replayed") is False, "candidate was not newly recorded")
        require(observation_result.get("replayed") is False, "observation was not newly recorded")
        require(experiment_result.get("replayed") is False, "experiment was not newly created")
        verifier.cli_json("experiment", "transition", experiment["id"], "draft", "ready")
        verifier.cli_json("experiment", "transition", experiment["id"], "ready", "running")
        verifier.cli_json(
            "experiment",
            "shadow",
            experiment["id"],
            os.fspath(write_fixture(fixture, "shadow.json", receipt)),
        )
        verifier.cli_json(
            "experiment",
            "benchmark",
            experiment["id"],
            os.fspath(write_fixture(fixture, "benchmark.json", benchmark)),
        )
        verifier.cli_json(
            "experiment", "transition", experiment["id"], "running", "awaiting-decision"
        )
        verifier.cli_json(
            "experiment",
            "decide",
            experiment["id"],
            os.fspath(write_fixture(fixture, "decision.json", decision)),
        )

    replayed_observation = verifier.mcp_tool(
        301, "mcg_experiments_mutate", {"action": "observation", "payload": observation}
    )
    replayed_decision = verifier.mcp_tool(
        302, "mcg_experiments_mutate", {"action": "decision", "payload": decision}
    )
    require(replayed_observation.get("replayed") is True, "MCP observation replay was not idempotent")
    require(replayed_decision.get("replayed") is True, "MCP decision replay was not idempotent")

    state = {
        "schema_version": 1,
        "organization_id": baseline["organization_id"],
        "baseline_revision_id": baseline["revision_id"],
        "candidate_revision_id": candidate["revision_id"],
        "experiment_id": experiment["id"],
        "observation_id": observation["id"],
        "task_id": task["id"],
        "benchmark_id": benchmark["id"],
        "decision_id": decision["id"],
    }
    state_path.parent.mkdir(parents=True, exist_ok=True)
    temporary_state = state_path.with_suffix(state_path.suffix + ".tmp")
    temporary_state.write_text(json.dumps(state, sort_keys=True), encoding="utf-8")
    os.replace(temporary_state, state_path)
    return verify(verifier, state)


def verify(verifier: InstalledExperimentVerifier, state: dict[str, Any]) -> dict[str, Any]:
    experiment_id = state["experiment_id"]
    organization_id = state["organization_id"]
    direct = verifier.request("GET", f"/api/v1/experiments/{experiment_id}")
    organization = verifier.request("GET", f"/api/v1/organizations/{organization_id}")
    cli = verifier.cli_json("experiment", "show", experiment_id)
    mcp = verifier.mcp_tool(
        401, "mcg_experiments_inspect", {"scope": "experiment", "id": experiment_id}
    )
    listed = verifier.cli_json("experiment", "list", organization_id)

    for surface, value in [("REST", direct), ("CLI", cli["experiment"]), ("MCP", mcp["experiment"])]:
        require(value["id"] == experiment_id, f"{surface} returned the wrong experiment")
        require(value["state"] == "promoted", f"{surface} did not retain promoted state")
        require(value["mode"] == "shadow", f"{surface} changed shadow mode")
    require(organization["revision_id"] == state["candidate_revision_id"], "promotion was not atomic")
    require(len(cli["shadow_executions"]) == 1, "CLI did not retain one shadow receipt")
    require(cli["shadow_executions"][0]["authority"] == "evaluation_only", "shadow gained authority")
    require(len(cli["benchmarks"]) == 1, "CLI did not retain one benchmark")
    require(cli["benchmarks"][0]["id"] == state["benchmark_id"], "CLI benchmark identity changed")
    require(len(cli["decisions"]) == 1, "CLI did not retain one decision")
    require(cli["decisions"][0]["benchmark_ids"] == [state["benchmark_id"]], "decision lost evidence")
    require(len(mcp["shadow_executions"]) == 1, "MCP did not retain one shadow receipt")
    require(len(mcp["benchmarks"]) == 1 and len(mcp["decisions"]) == 1, "MCP evidence is incomplete")
    require(
        any(value["id"] == experiment_id for value in listed["experiments"]),
        "installed CLI organization listing omitted the experiment",
    )
    return {
        "schemaVersion": 1,
        "verified": True,
        "experimentId": experiment_id,
        "organizationId": organization_id,
        "state": "promoted",
        "mode": "shadow",
        "shadowAuthority": "evaluation_only",
        "shadowReceiptCount": 1,
        "benchmarkCount": 1,
        "decisionCount": 1,
        "surfaces": ["rest", "cli", "mcp"],
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("exercise", "verify"), required=True)
    parser.add_argument("--host", required=True)
    parser.add_argument("--token-file", type=pathlib.Path, required=True)
    parser.add_argument("--cli", type=pathlib.Path, required=True)
    parser.add_argument("--state", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    arguments = parser.parse_args()
    verifier = InstalledExperimentVerifier(arguments.host, arguments.token_file, arguments.cli)
    if arguments.mode == "exercise":
        result = exercise(verifier, arguments.state)
    else:
        state = json.loads(arguments.state.read_text(encoding="utf-8"))
        result = verify(verifier, state)
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    temporary_output = arguments.output.with_suffix(arguments.output.suffix + ".tmp")
    temporary_output.write_text(json.dumps(result, indent=2, sort_keys=True), encoding="utf-8")
    os.replace(temporary_output, arguments.output)


if __name__ == "__main__":
    main()
