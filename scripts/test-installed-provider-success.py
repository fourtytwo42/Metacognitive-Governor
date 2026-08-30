#!/usr/bin/env python3
"""Verify successful provider-backed consumer work through an installed MCG CLI.

The exercise mode owns a deterministic loopback OpenAI-compatible provider, runs
human, JSON, and NDJSON consumer journeys, verifies governed project-command and
independent-evaluator evidence, and writes restart-safe state. Verify mode runs
after the packaged daemon restarts and proves that the same terminal evidence can
be inspected and replayed without a provider.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
import threading
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


SCHEMA_VERSION = 1
FIXTURE_MODEL = "installed-provider-success-model"
FIXTURE_TEXT = "installed provider-backed result"
JSON_FIXTURE_TEXT = "installed JSON conversation result"
STREAM_FIXTURE_TEXT = "installed streaming conversation result"
PRIVATE_CONTEXT = "bounded installed-package owner context"


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
        body = json.dumps(value, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
        if self.path != "/v1/models":
            self.send_json(404, {"error": "not found"})
            return
        self.send_json(
            200,
            {
                "object": "list",
                "data": [
                    {
                        "id": FIXTURE_MODEL,
                        "object": "model",
                        "owned_by": "mcg-installed-package-fixture",
                        "context_window": 32768,
                        "supported_parameters": ["tools"],
                    }
                ],
            },
        )

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
        if self.path != "/v1/responses":
            self.send_json(404, {"error": "not found"})
            return
        length = int(self.headers.get("Content-Length", "0"))
        request = json.loads(self.rfile.read(length))
        assert request.get("model") == FIXTURE_MODEL
        with self.server.call_lock:
            self.server.call_count += 1
            call_number = self.server.call_count
        self.send_json(
            200,
            {
                "id": f"installed-success-{call_number}",
                "model": FIXTURE_MODEL,
                "status": "completed",
                "output": [
                    {
                        "type": "message",
                        "role": "assistant",
                        "content": [{"type": "output_text", "text": FIXTURE_TEXT}],
                    }
                ],
                "usage": {"input_tokens": 24, "output_tokens": 4, "total_tokens": 28},
            },
        )


def api_json(
    host: str,
    token: str,
    method: str,
    path: str,
    body: object | None = None,
) -> Any:
    encoded = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        f"{host.rstrip('/')}{path}",
        data=encoded,
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
        detail = error.read().decode("utf-8", errors="replace")
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
    stdin: bytes = b"",
    timeout_seconds: int = 120,
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
        input=stdin,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout_seconds,
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
            f"{label} did not return JSON: {result.stdout.decode(errors='replace')}"
        ) from error


def require_success(result: subprocess.CompletedProcess[bytes], label: str) -> None:
    assert result.returncode == 0, (
        f"{label} exited {result.returncode}: "
        f"{result.stderr.decode(errors='replace')}"
    )


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
        cli,
        host,
        token_file,
        project,
        evidence_root,
        label,
        ["task", "audit", task_id],
    )
    require_success(result, label)
    records = decode_json(result, label)["records"]
    assert any(
        record.get("tool") == "process.exec"
        and record.get("phase") == "completion"
        and record.get("result") == "succeeded"
        for record in records
    ), f"{label} has no successful governed project command"
    return records


def relaunch_task(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    evidence_root: pathlib.Path,
    task_id: str,
    label: str,
) -> dict[str, Any]:
    result = invoke_cli(
        cli,
        host,
        token_file,
        project,
        evidence_root,
        label,
        ["task", "launch", task_id, "--json"],
    )
    require_success(result, label)
    launched = decode_json(result, label)
    assert launched["task"]["id"] == task_id
    assert launched["task"]["state"] == "completed"
    assert launched["replayed"] is True
    assert launched["execution"]["state"] == "completed"
    assert launched["run"]["run"]["state"] == "completed"
    assert launched["run"]["evaluations"][0]["outcome"] == "passed"
    assert len(launched["run"]["results"]) == 2
    return launched


def replay_run(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    evidence_root: pathlib.Path,
    run_id: str,
    label: str,
) -> dict[str, Any]:
    result = invoke_cli(
        cli,
        host,
        token_file,
        project,
        evidence_root,
        label,
        ["run", "replay", run_id, "--json"],
    )
    require_success(result, label)
    replay = decode_json(result, label)
    assert replay["run_id"] == run_id
    assert replay["side_effect_free"] is True
    assert replay["invariants"] and all(item["passed"] for item in replay["invariants"])
    return replay


def list_values(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    evidence_root: pathlib.Path,
    noun: str,
    label: str,
) -> list[dict[str, Any]]:
    result = invoke_cli(
        cli,
        host,
        token_file,
        project,
        evidence_root,
        label,
        [noun, "list", "--json"],
    )
    require_success(result, label)
    return decode_json(result, label)[f"{noun}s"]


def conversation_id_from_human_output(output: str) -> str:
    for line in output.splitlines():
        prefix = "Conversation: "
        if line.startswith(prefix):
            return str(uuid.UUID(line.removeprefix(prefix).strip()))
    raise AssertionError("provider-backed human output has no conversation identity")


def effective_batch_delegation(
    view: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    team = view["team"]
    batches = team["coordination_batches"]
    assert (
        len(batches) == 1
    ), "provider-backed human journey did not create one coordination batch"
    batch = batches[0]
    assert (
        batch["state"] == "completed"
    ), "provider-backed coordination batch is not terminal"
    delegations = [
        delegation
        for delegation in team["delegations"]
        if delegation.get("coordination_batch_id") == batch["id"]
    ]
    superseded_ids = {
        delegation["retry_of_id"]
        for delegation in delegations
        if delegation.get("retry_of_id") is not None
    }
    effective = [
        delegation
        for delegation in delegations
        if delegation["id"] not in superseded_ids
    ]
    assert (
        len(effective) == 1
    ), "provider-backed batch has no unique effective delegation"
    return batch, effective[0]


def assert_integrated_conversation(
    view: dict[str, Any],
    project_root: pathlib.Path,
    expected_path: str,
    expected_text: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    batch, delegation = effective_batch_delegation(view)
    assert delegation["state"] == "completed"
    assert delegation["integration_ready"] is True
    assert delegation["governance"]["outcome"]["successful"] is True
    assert delegation["unresolved_issues"] == []
    assert expected_path in delegation["changed_files"]
    assert (
        project_root.joinpath(expected_path).read_bytes()
        == f"{expected_text}\n".encode()
    )
    return batch, delegation


def create_and_launch_task(
    cli: pathlib.Path,
    host: str,
    token_file: pathlib.Path,
    project: pathlib.Path,
    evidence_root: pathlib.Path,
    project_id: str,
    goal: str,
    label: str,
) -> dict[str, Any]:
    created_result = invoke_cli(
        cli,
        host,
        token_file,
        project,
        evidence_root,
        f"{label}-create",
        ["task", "create", goal, "--project-id", project_id, "--json"],
    )
    require_success(created_result, f"{label}-create")
    created = decode_json(created_result, f"{label}-create")
    task_id = created["task"]["id"]
    launched_result = invoke_cli(
        cli,
        host,
        token_file,
        project,
        evidence_root,
        f"{label}-launch",
        ["task", "launch", task_id, "--json"],
    )
    require_success(launched_result, f"{label}-launch")
    launched = decode_json(launched_result, f"{label}-launch")
    assert launched["task"]["id"] == task_id
    assert launched["task"]["state"] == "completed"
    assert launched["execution"]["state"] == "completed"
    assert launched["run"]["run"]["state"] == "completed"
    assert launched["run"]["evaluations"][0]["outcome"] == "passed"
    assert len(launched["run"]["results"]) == 2
    return launched


def exercise(args: argparse.Namespace) -> None:
    cli = pathlib.Path(args.cli).resolve(strict=True)
    token_file = pathlib.Path(args.token_file).resolve(strict=True)
    project = pathlib.Path(args.project).resolve()
    state_path = pathlib.Path(args.state).resolve()
    output_path = pathlib.Path(args.output).resolve()
    evidence_root = output_path.parent
    evidence_root.mkdir(parents=True, exist_ok=True)
    project.mkdir(parents=True, exist_ok=True)
    project.joinpath("package.json").write_text(
        '{"scripts":{"test":"node --test"}}\n', encoding="utf-8"
    )
    token = token_file.read_text(encoding="utf-8").strip()
    assert token

    baseline_tasks = list_values(
        cli,
        args.host,
        token_file,
        project,
        evidence_root,
        "task",
        "provider-success-baseline-tasks",
    )
    baseline_projects = list_values(
        cli,
        args.host,
        token_file,
        project,
        evidence_root,
        "project",
        "provider-success-baseline-projects",
    )
    baseline_task_ids = {item["id"] for item in baseline_tasks}
    baseline_project_ids = {item["project_id"] for item in baseline_projects}

    provider = ProviderServer()
    provider_thread = threading.Thread(target=provider.serve_forever, daemon=True)
    provider_thread.start()
    provider_id = f"installed_success_{uuid.uuid4().hex}"
    provider_url = f"http://127.0.0.1:{provider.server_address[1]}/v1"
    configured = False
    captured_results: list[subprocess.CompletedProcess[bytes]] = []
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
                    "display_name": "Installed provider success fixture",
                    "kind": "open_ai_compatible",
                    "base_url": provider_url,
                    "auth_mode": "none",
                    "organization": None,
                    "project": None,
                },
                "credential": None,
            },
        )
        configured = True
        assert mutation["connection"]["id"] == provider_id
        assert mutation["catalog"]["models"][0]["id"] == FIXTURE_MODEL

        human = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "provider-success-human",
            [
                "In the linked project, create installed-provider-result.txt with "
                f"the exact UTF-8 content {FIXTURE_TEXT!r} followed by one newline, "
                "verify its exact bytes, and report completion."
            ],
            f"{PRIVATE_CONTEXT}\n".encode(),
            # This path runs the real Chat -> Coordinator -> Coder chain. Keep
            # short metadata and replay operations on the stricter default,
            # while allowing the governed multi-agent turn the same five-minute
            # envelope used by the representative project package UATs.
            timeout_seconds=300,
        )
        captured_results.append(human)
        require_success(human, "provider-success-human")
        human_text = human.stdout.decode("utf-8")
        human_text_folded = human_text.casefold()
        assert "review" in human_text_folded and "appl" in human_text_folded
        assert not project.joinpath("installed-provider-result.txt").exists()
        conversation_id = conversation_id_from_human_output(human_text)
        conversation = api_json(
            args.host,
            token,
            "GET",
            f"/api/v1/conversations/{conversation_id}",
        )
        batch, delegation = effective_batch_delegation(conversation)
        assert delegation["state"] == "reported"
        assert delegation["integration_ready"] is True
        assert delegation["governance"]["outcome"]["successful"] is True
        assert delegation["unresolved_issues"] == []
        assert len(delegation["report_sha256"]) == 64
        review = api_json(
            args.host,
            token,
            "GET",
            f"/api/v1/conversations/{conversation_id}/delegations/{delegation['id']}/review",
        )
        assert review["delegation"]["revision"] == delegation["revision"]
        assert "installed-provider-result.txt" in review["patch"]
        applied = api_json(
            args.host,
            token,
            "POST",
            f"/api/v1/conversations/{conversation_id}/delegations/{delegation['id']}/apply",
            {"expected_revision": review["delegation"]["revision"]},
        )
        applied_batch, applied_delegation = effective_batch_delegation(applied)
        assert applied_batch["id"] == batch["id"]
        assert applied_delegation["id"] == delegation["id"]
        assert applied_delegation["state"] == "completed"
        assert applied_delegation["governance"]["outcome"]["owner_interventions"] == 1
        applied_worktrees = [
            worktree
            for worktree in applied["team"]["worktrees"]
            if worktree.get("delegation_id") == delegation["id"]
        ]
        assert len(applied_worktrees) == 1
        assert applied_worktrees[0]["state"] == "removed"
        assert project.joinpath("installed-provider-result.txt").read_bytes() == (
            f"{FIXTURE_TEXT}\n".encode()
        )
        assert PRIVATE_CONTEXT not in human_text
        assert provider.call_count == 0

        projects = list_values(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "project",
            "provider-success-projects",
        )
        new_projects = [
            item for item in projects if item["project_id"] not in baseline_project_ids
        ]
        assert len(new_projects) == 1
        project_id = new_projects[0]["project_id"]

        structured = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "provider-success-json",
            [
                "run",
                "--json",
                "In the linked project, create installed-provider-json-result.txt with "
                f"the exact UTF-8 content {JSON_FIXTURE_TEXT!r} followed by one newline, "
                "verify its exact bytes, and report completion.",
            ],
            timeout_seconds=300,
        )
        captured_results.append(structured)
        require_success(structured, "provider-success-json")
        structured_value = decode_json(structured, "provider-success-json")
        assert structured_value["schema_version"] == 1
        assert structured_value["state"] == "completed"
        assert structured_value["hard_gate_ready"] is True
        structured_conversation_id = structured_value["conversation_id"]
        structured_conversation = api_json(
            args.host,
            token,
            "GET",
            f"/api/v1/conversations/{structured_conversation_id}",
        )
        _, structured_delegation = assert_integrated_conversation(
            structured_conversation,
            project,
            "installed-provider-json-result.txt",
            JSON_FIXTURE_TEXT,
        )

        stream = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "provider-success-stream",
            [
                "run",
                "--stream-json",
                "In the linked project, create installed-provider-stream-result.txt with "
                f"the exact UTF-8 content {STREAM_FIXTURE_TEXT!r} followed by one newline, "
                "verify its exact bytes, and report completion.",
            ],
            timeout_seconds=300,
        )
        captured_results.append(stream)
        require_success(stream, "provider-success-stream")
        records = [json.loads(line) for line in stream.stdout.splitlines() if line]
        assert records[0]["type"] == "thread.started"
        assert records[1]["type"] == "turn.started"
        assert records[-1]["type"] == "turn.completed"
        assert any(record["type"] == "item.completed" for record in records)
        stream_conversation_id = records[0]["thread_id"]
        stream_conversation = api_json(
            args.host,
            token,
            "GET",
            f"/api/v1/conversations/{stream_conversation_id}",
        )
        _, stream_delegation = assert_integrated_conversation(
            stream_conversation,
            project,
            "installed-provider-stream-result.txt",
            STREAM_FIXTURE_TEXT,
        )

        direct_launches = [
            create_and_launch_task(
                cli,
                args.host,
                token_file,
                project,
                evidence_root,
                project_id,
                "Complete and verify the installed deterministic provider JSON task",
                "provider-success-direct-json",
            ),
            create_and_launch_task(
                cli,
                args.host,
                token_file,
                project,
                evidence_root,
                project_id,
                "Complete and verify the installed deterministic provider streaming task",
                "provider-success-direct-stream",
            ),
        ]

        assert provider.call_count == 4
        tasks = list_values(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            "task",
            "provider-success-tasks",
        )
        new_tasks = [item for item in tasks if item["id"] not in baseline_task_ids]
        assert len(new_tasks) == 2
        assert all(item["state"] == "completed" for item in new_tasks)
        assert {item["task"]["id"] for item in direct_launches} == {
            item["id"] for item in new_tasks
        }

        audit_counts: dict[str, int] = {}
        run_ids: dict[str, str] = {}
        for index, task in enumerate(new_tasks, start=1):
            calls_before_relaunch = provider.call_count
            launched = relaunch_task(
                cli,
                args.host,
                token_file,
                project,
                evidence_root,
                task["id"],
                f"provider-success-relaunch-{index}",
            )
            assert provider.call_count == calls_before_relaunch
            run_ids[task["id"]] = launched["run"]["run"]["id"]
            records = task_audit(
                cli,
                args.host,
                token_file,
                project,
                evidence_root,
                task["id"],
                f"provider-success-audit-{index}",
            )
            audit_counts[task["id"]] = len(records)

        for result in captured_results:
            assert (
                token.encode() not in result.stdout
                and token.encode() not in result.stderr
            )
            assert PRIVATE_CONTEXT.encode() not in result.stdout

        state = {
            "schema_version": SCHEMA_VERSION,
            "task_ids": sorted(item["id"] for item in new_tasks),
            "project_id": project_id,
            "project_path_sha256": hashlib.sha256(
                str(project).encode("utf-8")
            ).hexdigest(),
            "provider_calls": provider.call_count,
            "audit_counts": audit_counts,
            "run_ids": run_ids,
            "conversation_id": conversation_id,
            "delegation_id": delegation["id"],
            "report_sha256": delegation["report_sha256"],
            "conversation_results": [
                {
                    "conversation_id": structured_conversation_id,
                    "delegation_id": structured_delegation["id"],
                    "report_sha256": structured_delegation["report_sha256"],
                    "path": "installed-provider-json-result.txt",
                    "text": JSON_FIXTURE_TEXT,
                },
                {
                    "conversation_id": stream_conversation_id,
                    "delegation_id": stream_delegation["id"],
                    "report_sha256": stream_delegation["report_sha256"],
                    "path": "installed-provider-stream-result.txt",
                    "text": STREAM_FIXTURE_TEXT,
                },
            ],
        }
        state_path.write_text(json.dumps(state, sort_keys=True), encoding="utf-8")
        output_path.write_text(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "mode": "exercise",
                    "human_completed": True,
                    "json_completed": True,
                    "stream_completed": True,
                    "independent_evaluation_passed": True,
                    "governed_project_command_per_task": True,
                    "replay_side_effect_free": True,
                    "provider_calls": provider.call_count,
                    "task_count": len(new_tasks),
                    "project_count": len(new_projects),
                    "token_excluded": True,
                    "private_context_excluded": True,
                    "accepted_specialist_report": True,
                    "terminal_coordination_batch": True,
                    "owner_apply_completed": True,
                    "isolated_worktree_removed": True,
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
    token = token_file.read_text(encoding="utf-8").strip()
    state = json.loads(state_path.read_text(encoding="utf-8"))
    assert state["schema_version"] == SCHEMA_VERSION
    assert (
        state["project_path_sha256"]
        == hashlib.sha256(str(project).encode("utf-8")).hexdigest()
    )
    conversation = api_json(
        args.host,
        token,
        "GET",
        f"/api/v1/conversations/{state['conversation_id']}",
    )
    batch, delegation = effective_batch_delegation(conversation)
    assert batch["state"] == "completed"
    assert delegation["id"] == state["delegation_id"]
    assert delegation["state"] == "completed"
    assert delegation["report_sha256"] == state["report_sha256"]
    worktrees = [
        worktree
        for worktree in conversation["team"]["worktrees"]
        if worktree.get("delegation_id") == state["delegation_id"]
    ]
    assert len(worktrees) == 1 and worktrees[0]["state"] == "removed"
    assert project.joinpath("installed-provider-result.txt").read_bytes() == (
        f"{FIXTURE_TEXT}\n".encode()
    )

    for result in state["conversation_results"]:
        view = api_json(
            args.host,
            token,
            "GET",
            f"/api/v1/conversations/{result['conversation_id']}",
        )
        _, persisted_delegation = assert_integrated_conversation(
            view,
            project,
            result["path"],
            result["text"],
        )
        assert persisted_delegation["id"] == result["delegation_id"]
        assert persisted_delegation["report_sha256"] == result["report_sha256"]

    for index, task_id in enumerate(state["task_ids"], start=1):
        result = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            f"provider-success-restart-task-{index}",
            ["task", "get", task_id, "--json"],
        )
        require_success(result, f"provider-success-restart-task-{index}")
        task = decode_json(result, f"provider-success-restart-task-{index}")
        assert task["id"] == task_id and task["state"] == "completed"
        launched = relaunch_task(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            task_id,
            f"provider-success-restart-relaunch-{index}",
        )
        assert launched["run"]["run"]["id"] == state["run_ids"][task_id]
        records = task_audit(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            task_id,
            f"provider-success-restart-audit-{index}",
        )
        assert len(records) == state["audit_counts"][task_id]

    for index, run_id in enumerate(state["run_ids"].values(), start=1):
        run = invoke_cli(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            f"provider-success-restart-run-{index}",
            ["run", "get", run_id],
        )
        require_success(run, f"provider-success-restart-run-{index}")
        run_value = decode_json(run, f"provider-success-restart-run-{index}")
        assert run_value["run"]["state"] == "completed"
        assert run_value["evaluations"][0]["outcome"] == "passed"
        assert len(run_value["results"]) == 2
        replay_run(
            cli,
            args.host,
            token_file,
            project,
            evidence_root,
            run_id,
            f"provider-success-restart-replay-{index}",
        )
        assert token.encode() not in run.stdout and token.encode() not in run.stderr

    output_path.write_text(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "mode": "verify",
                "restart_tasks_completed": len(state["task_ids"]),
                "restart_runs_completed": len(state["run_ids"]),
                "restart_evaluation_passed": True,
                "restart_audit_consistent": True,
                "restart_replay_side_effect_free": True,
                "provider_required_for_replay": False,
                "token_excluded": True,
                "accepted_specialist_report_survived_restart": True,
                "owner_apply_survived_restart": True,
                "json_conversation_survived_restart": True,
                "stream_conversation_survived_restart": True,
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
