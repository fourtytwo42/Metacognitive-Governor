#!/usr/bin/env python3
"""Qualify explicit cross-project memory transfer through an installed MCG CLI.

Exercise mode creates two disposable projects, promotes one source claim to
durable memory, previews and transfers it, and proves cross-interface replay and
conflict behavior. Verify mode runs after encrypted restore and daemon restart
and proves that the immutable claim/transfer history survived without a duplicate
claim or event.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
import urllib.error
import urllib.request
import uuid
from typing import Any


SCHEMA_VERSION = 1
NAMESPACE = "installed_qualification"
KEY = "package_manager"
VALUE = "pnpm"
REASON = "Owner reviewed this package-manager convention for the target project"


def invoke_cli(
    args: argparse.Namespace,
    evidence_root: pathlib.Path,
    label: str,
    command: list[str],
) -> subprocess.CompletedProcess[bytes]:
    result = subprocess.run(
        [
            str(pathlib.Path(args.cli).resolve(strict=True)),
            "--host",
            args.host,
            "--token-file",
            str(pathlib.Path(args.token_file).resolve(strict=True)),
            "-C",
            str(pathlib.Path(args.workspace).resolve(strict=True)),
            *command,
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=120,
        check=False,
    )
    evidence_root.joinpath(f"memory-transfer-{label}.stdout").write_bytes(result.stdout)
    evidence_root.joinpath(f"memory-transfer-{label}.stderr").write_bytes(result.stderr)
    return result


def require_success(result: subprocess.CompletedProcess[bytes], label: str) -> None:
    assert result.returncode == 0, (
        f"{label} exited {result.returncode}: {result.stderr.decode(errors='replace')}"
    )


def decode_json(result: subprocess.CompletedProcess[bytes], label: str) -> Any:
    require_success(result, label)
    try:
        return json.loads(result.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AssertionError(
            f"{label} did not return JSON: {result.stdout.decode(errors='replace')}"
        ) from error


def api_json(args: argparse.Namespace, method: str, path: str) -> Any:
    token = pathlib.Path(args.token_file).read_text(encoding="utf-8").strip()
    request = urllib.request.Request(
        f"{args.host.rstrip('/')}{path}",
        headers={"Authorization": f"Bearer {token}"},
        method=method,
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def mcp_call(args: argparse.Namespace, name: str, arguments: dict[str, Any]) -> Any:
    token = pathlib.Path(args.token_file).read_text(encoding="utf-8").strip()
    payload = {
        "jsonrpc": "2.0",
        "id": 91,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments,
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientInfo": {
                    "name": "mcg-installed-memory-transfer-uat",
                    "version": "1.0.0",
                },
                "io.modelcontextprotocol/clientCapabilities": {},
            },
        },
    }
    request = urllib.request.Request(
        f"{args.host.rstrip('/')}/mcp",
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
            "MCP-Protocol-Version": "2026-07-28",
            "Mcp-Method": "tools/call",
            "Mcp-Name": name,
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        value = json.load(response)
    assert "error" not in value, value
    return value["result"]["structuredContent"]


def project_add(
    args: argparse.Namespace,
    evidence_root: pathlib.Path,
    label: str,
    root: pathlib.Path,
) -> str:
    result = invoke_cli(args, evidence_root, label, ["project", "add", str(root), "--json"])
    return decode_json(result, label)["project"]["project_id"]


def memory_command(
    args: argparse.Namespace,
    evidence_root: pathlib.Path,
    label: str,
    project_id: str,
    command: list[str],
) -> subprocess.CompletedProcess[bytes]:
    return invoke_cli(
        args,
        evidence_root,
        label,
        ["project", "memory", project_id, *command],
    )


def transition_id(result: subprocess.CompletedProcess[bytes], label: str) -> str:
    require_success(result, label)
    return result.stdout.decode("utf-8").split(":", maxsplit=1)[0].strip()


def assert_transfer_state(
    args: argparse.Namespace,
    evidence_root: pathlib.Path,
    state: dict[str, Any],
    prefix: str,
) -> None:
    preview = decode_json(
        memory_command(
            args,
            evidence_root,
            f"{prefix}-preview",
            state["source_project_id"],
            ["transfer-preview", state["source_claim_id"]],
        ),
        f"{prefix}-preview",
    )
    assert preview["source_claim_sha256"] == state["source_claim_sha256"]
    assert preview["claim"]["state"] == "durable"

    target = api_json(
        args,
        "GET",
        f"/api/v1/projects/{state['target_project_id']}/memory",
    )
    matching = [claim for claim in target["claims"] if claim["id"] == state["target_claim_id"]]
    assert len(matching) == 1
    claim = matching[0]
    assert claim["namespace"] == NAMESPACE and claim["key"] == KEY
    assert claim["value"] == VALUE and claim["state"] == "new"

    for direction, project_id in (
        ("outbound", state["source_project_id"]),
        ("inbound", state["target_project_id"]),
    ):
        transfers = decode_json(
            memory_command(
                args,
                evidence_root,
                f"{prefix}-{direction}",
                project_id,
                ["transfers"],
            ),
            f"{prefix}-{direction}",
        )["transfers"]
        matching_transfers = [item for item in transfers if item["id"] == state["transfer_id"]]
        assert len(matching_transfers) == 1
        assert matching_transfers[0]["source_claim_sha256"] == state["source_claim_sha256"]


def exercise(args: argparse.Namespace) -> None:
    workspace = pathlib.Path(args.workspace).resolve(strict=True)
    state_path = pathlib.Path(args.state).resolve()
    output_path = pathlib.Path(args.output).resolve()
    evidence_root = output_path.parent
    evidence_root.mkdir(parents=True, exist_ok=True)
    source_root = workspace / "memory-transfer-source"
    target_root = workspace / "memory-transfer-target"
    source_root.mkdir(parents=True, exist_ok=True)
    target_root.mkdir(parents=True, exist_ok=True)
    source_root.joinpath("package.json").write_text("{}\n", encoding="utf-8")
    target_root.joinpath("Cargo.toml").write_text("[workspace]\n", encoding="utf-8")

    source_project_id = project_add(args, evidence_root, "add-source", source_root)
    target_project_id = project_add(args, evidence_root, "add-target", target_root)
    created = memory_command(
        args,
        evidence_root,
        "add-claim",
        source_project_id,
        ["add", NAMESPACE, KEY, json.dumps(VALUE), "--confidence", "0.9", "--source", "installed owner fixture"],
    )
    require_success(created, "add-claim")
    claim_id = created.stdout.decode("utf-8").strip()
    for index in (1, 2):
        claim_id = transition_id(
            memory_command(
                args,
                evidence_root,
                f"reinforce-{index}",
                source_project_id,
                ["transition", claim_id, "reinforce", "--source", "installed owner fixture"],
            ),
            f"reinforce-{index}",
        )

    preview = decode_json(
        memory_command(
            args,
            evidence_root,
            "preview",
            source_project_id,
            ["transfer-preview", claim_id],
        ),
        "preview",
    )
    transfer_id = str(uuid.uuid4())
    transfer = decode_json(
        memory_command(
            args,
            evidence_root,
            "transfer",
            source_project_id,
            [
                "transfer",
                claim_id,
                target_project_id,
                "--source-sha256",
                preview["source_claim_sha256"],
                "--reason",
                REASON,
                "--transfer-id",
                transfer_id,
            ],
        ),
        "transfer",
    )
    assert transfer["replayed"] is False and len(transfer["events"]) == 1
    assert transfer["transfer"]["id"] == transfer_id
    assert transfer["claim"]["project_id"] == target_project_id

    replay = mcp_call(
        args,
        "mcg_project_memory_transfer",
        {
            "transfer_id": transfer_id,
            "source_project_id": source_project_id,
            "source_claim_id": claim_id,
            "source_claim_sha256": preview["source_claim_sha256"],
            "target_project_id": target_project_id,
            "reason": REASON,
        },
    )
    assert replay["replayed"] is True and replay["events"] == []
    assert replay["claim"]["id"] == transfer["claim"]["id"]

    changed = memory_command(
        args,
        evidence_root,
        "changed-replay",
        source_project_id,
        [
            "transfer",
            claim_id,
            target_project_id,
            "--source-sha256",
            preview["source_claim_sha256"],
            "--reason",
            "Changed review reason",
            "--transfer-id",
            transfer_id,
        ],
    )
    assert changed.returncode != 0

    state = {
        "schema_version": SCHEMA_VERSION,
        "workspace_sha256": hashlib.sha256(str(workspace).encode("utf-8")).hexdigest(),
        "source_project_id": source_project_id,
        "target_project_id": target_project_id,
        "source_claim_id": claim_id,
        "source_claim_sha256": preview["source_claim_sha256"],
        "target_claim_id": transfer["claim"]["id"],
        "transfer_id": transfer_id,
    }
    assert_transfer_state(args, evidence_root, state, "exercise")
    state_path.write_text(json.dumps(state, sort_keys=True), encoding="utf-8")
    output_path.write_text(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "mode": "exercise",
                "source_became_durable": True,
                "explicit_preview": True,
                "cli_transfer_created": True,
                "mcp_exact_replay": True,
                "changed_replay_failed_closed": True,
                "inbound_and_outbound_history": True,
                "target_claim_count": 1,
            },
            sort_keys=True,
        ),
        encoding="utf-8",
    )


def verify(args: argparse.Namespace) -> None:
    workspace = pathlib.Path(args.workspace).resolve(strict=True)
    state = json.loads(pathlib.Path(args.state).resolve(strict=True).read_text(encoding="utf-8"))
    assert state["schema_version"] == SCHEMA_VERSION
    assert state["workspace_sha256"] == hashlib.sha256(str(workspace).encode("utf-8")).hexdigest()
    output_path = pathlib.Path(args.output).resolve()
    evidence_root = output_path.parent
    assert_transfer_state(args, evidence_root, state, "restart")

    replay = mcp_call(
        args,
        "mcg_project_memory_transfer",
        {
            "transfer_id": state["transfer_id"],
            "source_project_id": state["source_project_id"],
            "source_claim_id": state["source_claim_id"],
            "source_claim_sha256": state["source_claim_sha256"],
            "target_project_id": state["target_project_id"],
            "reason": REASON,
        },
    )
    assert replay["replayed"] is True and replay["events"] == []
    assert replay["claim"]["id"] == state["target_claim_id"]
    assert_transfer_state(args, evidence_root, state, "post-replay")
    output_path.write_text(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "mode": "verify",
                "encrypted_restore_survived": True,
                "daemon_restart_survived": True,
                "exact_replay_created_no_event": True,
                "target_claim_count": 1,
                "transfer_history_count": 1,
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
    parser.add_argument("--workspace", required=True)
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
