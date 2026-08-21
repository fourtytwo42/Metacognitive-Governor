#!/usr/bin/env python3
"""Qualify successful work through installed REST, OpenAI, and MCP surfaces.

Exercise mode owns one credential-free loopback provider and crosses direct REST
project discovery, buffered/streaming Responses, Chat Completions, and the complete
MCP consumer-task lifecycle. Verify mode runs after encrypted restore and daemon
restart with no provider and proves content-free proxy history plus exact MCP replay.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import threading
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


SCHEMA_VERSION = 1
MCP_VERSION = "2026-07-28"
FIXTURE_MODEL = "installed-interface-success-model"
FIXTURE_TEXT = "installed interface success result"
PRIVATE_INPUT = "private installed interface prompt marker"


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
                        "owned_by": "mcg-installed-interface-fixture",
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
        if request.get("stream") is True:
            response_id = f"installed-interface-stream-{call_number}"
            completed = {
                "type": "response.completed",
                "response": {
                    "id": response_id,
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
            }
            events = (
                "event: response.created\n"
                f'data: {json.dumps({"type": "response.created", "response": {"id": response_id, "model": FIXTURE_MODEL}}, separators=(",", ":"))}\n\n'
                "event: response.output_text.delta\n"
                f'data: {json.dumps({"type": "response.output_text.delta", "item_id": "message-1", "delta": FIXTURE_TEXT}, separators=(",", ":"))}\n\n'
                "event: response.completed\n"
                f'data: {json.dumps(completed, separators=(",", ":"))}\n\n'
            ).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(events)))
            self.end_headers()
            self.wfile.write(events)
            return
        self.send_json(
            200,
            {
                "id": f"installed-interface-{call_number}",
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


def request(
    args: argparse.Namespace,
    method: str,
    path: str,
    body: object | None = None,
    accept: str = "application/json",
) -> tuple[int, dict[str, str], bytes]:
    token = pathlib.Path(args.token_file).read_text(encoding="utf-8").strip()
    encoded = None if body is None else json.dumps(body, separators=(",", ":")).encode("utf-8")
    http_request = urllib.request.Request(
        f"{args.host.rstrip('/')}{path}",
        data=encoded,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": accept,
        },
        method=method,
    )
    try:
        with urllib.request.urlopen(http_request, timeout=120) as response:
            return response.status, dict(response.headers.items()), response.read()
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")
        raise AssertionError(f"{method} {path} failed with HTTP {error.code}: {detail}") from error


def api_json(
    args: argparse.Namespace,
    method: str,
    path: str,
    body: object | None = None,
) -> Any:
    status, _headers, payload = request(args, method, path, body)
    assert 200 <= status < 300
    return None if not payload else json.loads(payload)


def mcp_tool(
    args: argparse.Namespace,
    request_id: int,
    name: str,
    arguments: dict[str, Any],
) -> dict[str, Any]:
    token = pathlib.Path(args.token_file).read_text(encoding="utf-8").strip()
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
                    "name": "mcg-installed-interface-success-uat",
                    "version": "1.0.0",
                },
                "io.modelcontextprotocol/clientCapabilities": {},
            },
        },
    }
    http_request = urllib.request.Request(
        f"{args.host.rstrip('/')}/mcp",
        data=json.dumps(payload, separators=(",", ":")).encode("utf-8"),
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
    with urllib.request.urlopen(http_request, timeout=120) as response:
        value = json.load(response)
    assert "error" not in value, value
    assert value["result"].get("isError") is not True, value
    return value["result"]["structuredContent"]


def assert_proxy_history(args: argparse.Namespace, state: dict[str, Any]) -> None:
    conversations = api_json(args, "GET", "/api/v1/provider-proxy/conversations")
    encoded = json.dumps(conversations, separators=(",", ":"))
    assert PRIVATE_INPUT not in encoded
    assert pathlib.Path(args.token_file).read_text(encoding="utf-8").strip() not in encoded
    ids = {conversation["id"] for conversation in conversations["conversations"]}
    assert set(state["conversation_ids"]).issubset(ids)
    for conversation_id in state["conversation_ids"]:
        conversation = api_json(
            args,
            "GET",
            f"/api/v1/provider-proxy/conversations/{conversation_id}",
        )
        encoded_conversation = json.dumps(conversation, separators=(",", ":"))
        assert PRIVATE_INPUT not in encoded_conversation
        assert conversation["conversation"]["id"] == conversation_id
        assert len(conversation["calls"]) == 1
        assert conversation["calls"][0]["outcome"] == "completed"


def exercise(args: argparse.Namespace) -> None:
    workspace = pathlib.Path(args.workspace).resolve(strict=True)
    state_path = pathlib.Path(args.state).resolve()
    output_path = pathlib.Path(args.output).resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    project = workspace / "interface-success-project"
    project.mkdir(parents=True, exist_ok=True)
    project.joinpath("README.md").write_text("# Installed interface success fixture\n", encoding="utf-8")
    token = pathlib.Path(args.token_file).read_text(encoding="utf-8").strip()
    assert token

    provider = ProviderServer()
    provider_thread = threading.Thread(target=provider.serve_forever, daemon=True)
    provider_thread.start()
    provider_id = f"installed_interfaces_{uuid.uuid4().hex}"
    configured = False
    try:
        configured_provider = api_json(
            args,
            "POST",
            "/api/v1/providers",
            {
                "connection": {
                    "contract_version": 1,
                    "id": provider_id,
                    "display_name": "Installed interface success fixture",
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
        assert configured_provider["connection"]["id"] == provider_id
        assert configured_provider["catalog"]["models"][0]["id"] == FIXTURE_MODEL

        project_id = str(uuid.uuid4())
        discovered = api_json(
            args,
            "POST",
            "/api/v1/projects",
            {"root": str(project), "project_id": project_id},
        )
        assert discovered["project"]["project_id"] == project_id

        models = api_json(args, "GET", "/v1/models")
        aliases = {model["id"]: model for model in models["data"]}
        assert aliases["mcg:auto"]["metadata"]["available"] is True
        assert aliases["mcg:local"]["metadata"]["available"] is True

        response_conversation = str(uuid.uuid4())
        response = api_json(
            args,
            "POST",
            "/v1/responses",
            {
                "model": "mcg:auto",
                "input": PRIVATE_INPUT,
                "metadata": {"mcg_conversation_id": response_conversation},
            },
        )
        assert response["output"][0]["content"][0]["text"] == FIXTURE_TEXT
        assert response["mcg_route"]["provider_id"] == provider_id
        assert response["usage"]["total_tokens"] == 28
        assert response["mcg_ledger"]["conversation_id"] == response_conversation

        chat_conversation = str(uuid.uuid4())
        chat = api_json(
            args,
            "POST",
            "/v1/chat/completions",
            {
                "model": "mcg:local",
                "messages": [{"role": "user", "content": PRIVATE_INPUT}],
                "metadata": {"mcg_conversation_id": chat_conversation},
            },
        )
        assert chat["object"] == "chat.completion"
        assert chat["choices"][0]["message"]["content"] == FIXTURE_TEXT

        stream_conversation = str(uuid.uuid4())
        status, headers, stream_body = request(
            args,
            "POST",
            "/v1/responses",
            {
                "model": "mcg:auto",
                "input": PRIVATE_INPUT,
                "stream": True,
                "metadata": {"mcg_conversation_id": stream_conversation},
            },
            "text/event-stream",
        )
        content_type = next(
            (value for key, value in headers.items() if key.lower() == "content-type"),
            "",
        )
        assert status == 200 and content_type.startswith("text/event-stream")
        stream = stream_body.decode("utf-8")
        assert "response.output_text.delta" in stream, stream
        assert "response.completed" in stream, stream
        assert FIXTURE_TEXT in stream

        task_arguments = {
            "request_key": f"installed-interface-{uuid.uuid4()}",
            "project_id": project_id,
            "goal": "Verify the installed interface fixture readme",
            "success_conditions": ["FILE EXISTS README.md"],
            "context_token_budget": 512,
            "estimated_output_tokens": 128,
            "max_model_calls": 2,
        }
        completed = mcp_tool(args, 201, "mcg_run_task", task_arguments)
        assert completed["replayed"] is False
        assert completed["task"]["state"] == "completed"
        assert completed["execution"]["state"] == "completed"
        assert completed["run"]["run"]["state"] == "completed"
        assert completed["run"]["evaluations"][0]["outcome"] == "passed"
        assert len(completed["run"]["results"]) == 2
        calls_before_replay = provider.call_count
        replay = mcp_tool(args, 202, "mcg_run_task", task_arguments)
        assert replay["replayed"] is True
        assert replay["task"]["id"] == completed["task"]["id"]
        assert provider.call_count == calls_before_replay
        assert provider.call_count == 5

        queried = mcp_tool(
            args,
            203,
            "mcg_query_task",
            {"task_id": completed["task"]["id"], "limit": 100},
        )
        assert queried["task"]["state"] == "completed"
        assert queried["run"]["evaluations"][0]["outcome"] == "passed"
        assert_proxy_history(
            args,
            {"conversation_ids": [response_conversation, chat_conversation, stream_conversation]},
        )

        state = {
            "schema_version": SCHEMA_VERSION,
            "project_id": project_id,
            "task_id": completed["task"]["id"],
            "run_id": completed["run"]["run"]["id"],
            "request_key": task_arguments["request_key"],
            "conversation_ids": [response_conversation, chat_conversation, stream_conversation],
            "provider_calls": provider.call_count,
        }
        state_path.write_text(json.dumps(state, sort_keys=True), encoding="utf-8")
        output_path.write_text(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "mode": "exercise",
                    "direct_rest_project_discovery": True,
                    "responses_completed": True,
                    "chat_completed": True,
                    "responses_stream_completed": True,
                    "mcp_task_completed": True,
                    "mcp_exact_replay": True,
                    "independent_evaluation_passed": True,
                    "content_free_history": True,
                    "provider_calls": provider.call_count,
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
    finally:
        if configured:
            api_json(args, "DELETE", f"/api/v1/providers/{provider_id}")
        provider.shutdown()
        provider.server_close()
        provider_thread.join(timeout=5)


def verify(args: argparse.Namespace) -> None:
    state = json.loads(pathlib.Path(args.state).resolve(strict=True).read_text(encoding="utf-8"))
    assert state["schema_version"] == SCHEMA_VERSION
    assert state["provider_calls"] == 5
    assert_proxy_history(args, state)

    replay = mcp_tool(
        args,
        301,
        "mcg_run_task",
        {
            "request_key": state["request_key"],
            "project_id": state["project_id"],
            "goal": "Verify the installed interface fixture readme",
            "success_conditions": ["FILE EXISTS README.md"],
            "context_token_budget": 512,
            "estimated_output_tokens": 128,
            "max_model_calls": 2,
        },
    )
    assert replay["replayed"] is True
    assert replay["task"]["id"] == state["task_id"]
    assert replay["run"]["run"]["id"] == state["run_id"]
    assert replay["task"]["state"] == "completed"
    assert replay["run"]["evaluations"][0]["outcome"] == "passed"
    queried = mcp_tool(
        args,
        302,
        "mcg_query_task",
        {"task_id": state["task_id"], "limit": 100},
    )
    assert queried["task"]["state"] == "completed"
    assert queried["run"]["run"]["id"] == state["run_id"]
    output_path = pathlib.Path(args.output).resolve()
    output_path.write_text(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "mode": "verify",
                "encrypted_restore_survived": True,
                "daemon_restart_survived": True,
                "proxy_history_content_free": True,
                "provider_required_for_mcp_replay": False,
                "mcp_task_completed": True,
                "independent_evaluation_passed": True,
                "conversation_count": len(state["conversation_ids"]),
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
