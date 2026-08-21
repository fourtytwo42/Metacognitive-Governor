#!/usr/bin/env python3
"""Qualify the installed Linux desktop through the native AT-SPI bridge."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Iterable

CANARY = "MCG_NATIVE_SECRET_CANARY"
INTERACTIVE_ROLES = {
    "check box",
    "combo box",
    "entry",
    "link",
    "password text",
    "push button",
    "radio button",
    "slider",
    "spin button",
    "text",
    "toggle button",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--application", default="mcg-desktop")
    parser.add_argument("--timeout-seconds", type=float, default=15.0)
    return parser.parse_args()


def children(node: Any) -> Iterable[Any]:
    try:
        yield from node
    except Exception:
        return


def walk(node: Any, *, budget: int = 2_000) -> list[Any]:
    pending = [node]
    found: list[Any] = []
    while pending:
        current = pending.pop()
        found.append(current)
        if len(found) > budget:
            raise RuntimeError("AT-SPI tree exceeded the 2000-node safety bound")
        pending.extend(reversed(list(children(current))))
    return found


def role_name(node: Any) -> str:
    try:
        return str(node.getRoleName() or "")
    except Exception:
        return ""


def accessible_name(node: Any) -> str:
    try:
        return str(node.name or "")
    except Exception:
        return ""


def has_state(node: Any, state: Any) -> bool:
    try:
        return bool(node.getState().contains(state))
    except Exception:
        return False


def find_node(nodes: Iterable[Any], role: str, names: set[str]) -> Any | None:
    for node in nodes:
        if role_name(node) == role and accessible_name(node) in names:
            return node
    return None


def wait_for_node(app: Any, role: str, names: set[str], deadline: float) -> Any:
    while time.monotonic() < deadline:
        node = find_node(walk(app), role, names)
        if node is not None:
            return node
        time.sleep(0.1)
    expected = ", ".join(sorted(names))
    raise RuntimeError(f"AT-SPI did not expose {role} named one of: {expected}")


def wait_for_focused(app: Any, name: str, focused_state: Any, deadline: float) -> Any:
    while time.monotonic() < deadline:
        for node in walk(app):
            if accessible_name(node) == name and has_state(node, focused_state):
                return node
        time.sleep(0.05)
    raise RuntimeError(f"AT-SPI focus did not reach {name!r}")


def click_accessible(node: Any, atspi: Any) -> None:
    component = node.queryComponent()
    if not component.scrollTo(atspi.SCROLL_ANYWHERE):
        raise RuntimeError(f"AT-SPI could not scroll {accessible_name(node)!r} into view")
    time.sleep(0.1)
    box = component.getExtents(atspi.DESKTOP_COORDS)
    if box.width < 2 or box.height < 2:
        raise RuntimeError(f"AT-SPI exposed invalid bounds for {accessible_name(node)!r}")
    subprocess.run(
        [
            "xdotool",
            "mousemove",
            str(box.x + box.width // 2),
            str(box.y + box.height // 2),
            "click",
            "1",
        ],
        check=True,
        timeout=5,
    )


def main() -> int:
    args = parse_args()
    if sys.platform != "linux":
        raise RuntimeError("The AT-SPI qualification must run on Linux")
    if not os.environ.get("DISPLAY") or not os.environ.get("DBUS_SESSION_BUS_ADDRESS"):
        raise RuntimeError("DISPLAY and DBUS_SESSION_BUS_ADDRESS are required")
    try:
        import pyatspi  # type: ignore[import-not-found]
    except ImportError as error:
        raise RuntimeError("python3-pyatspi is required") from error

    deadline = time.monotonic() + args.timeout_seconds
    desktop = pyatspi.Registry.getDesktop(0)
    app = None
    while time.monotonic() < deadline:
        for candidate in children(desktop):
            if accessible_name(candidate).casefold() == args.application.casefold():
                app = candidate
                break
        if app is not None:
            break
        time.sleep(0.1)
    if app is None:
        raise RuntimeError(f"AT-SPI did not expose application {args.application!r}")

    nodes = walk(app)
    required = {
        "frame": find_node(nodes, "frame", {"MCG"}),
        "document": find_node(nodes, "document web", {"MCG"}),
        "primary_navigation": find_node(nodes, "landmark", {"Primary navigation"}),
        "goal": find_node(nodes, "entry", {"Goal"}),
        "open_folder": find_node(nodes, "push button", {"Open folder"}),
        "create_task": find_node(nodes, "push button", {"Create task"}),
        "openai_oauth": find_node(
            nodes,
            "push button",
            {"Continue with OpenAI", "Connect with OpenAI"},
        ),
        "add_provider": find_node(nodes, "push button", {"Add API or local provider"}),
        "settings": find_node(nodes, "landmark", {"Settings"}),
    }
    missing = [name for name, node in required.items() if node is None]
    if missing:
        raise RuntimeError(f"AT-SPI is missing required native controls: {', '.join(missing)}")

    focusable_required = ["goal", "open_folder", "create_task", "openai_oauth", "add_provider"]
    not_focusable = [
        name for name in focusable_required if not has_state(required[name], pyatspi.STATE_FOCUSABLE)
    ]
    if not_focusable:
        raise RuntimeError(f"Required controls are not keyboard-focusable: {', '.join(not_focusable)}")

    unnamed_focusable = sorted(
        {
            role_name(node)
            for node in nodes
            if role_name(node) in INTERACTIVE_ROLES
            and has_state(node, pyatspi.STATE_FOCUSABLE)
            and not accessible_name(node).strip()
        }
    )
    if unnamed_focusable:
        raise RuntimeError(
            "AT-SPI exposed unnamed focusable controls for roles: " + ", ".join(unnamed_focusable)
        )

    goal = required["goal"]
    assert goal is not None
    goal.queryComponent().scrollTo(pyatspi.SCROLL_ANYWHERE)
    if not goal.queryComponent().grabFocus():
        raise RuntimeError("AT-SPI could not focus the Goal control")
    wait_for_focused(app, "Goal", pyatspi.STATE_FOCUSED, deadline)
    subprocess.run(["xdotool", "key", "Tab"], check=True, timeout=5)
    wait_for_focused(app, "Open folder", pyatspi.STATE_FOCUSED, deadline)
    subprocess.run(["xdotool", "key", "Tab"], check=True, timeout=5)
    wait_for_focused(app, "Create task", pyatspi.STATE_FOCUSED, deadline)

    mode = wait_for_node(app, "toggle button", {"Advanced mode"}, deadline)
    if not has_state(mode, pyatspi.STATE_FOCUSABLE):
        raise RuntimeError("The mode switch is not keyboard-focusable through AT-SPI")
    click_accessible(mode, pyatspi)
    wait_for_node(app, "toggle button", {"Simple mode"}, deadline)

    add_provider = wait_for_node(app, "push button", {"Add API or local provider"}, deadline)
    click_accessible(add_provider, pyatspi)
    password = wait_for_node(app, "password text", {"API key"}, deadline)
    if not has_state(password, pyatspi.STATE_FOCUSABLE):
        raise RuntimeError("The API key password control is not keyboard-focusable")
    click_accessible(password, pyatspi)
    subprocess.run(
        ["xdotool", "type", "--clearmodifiers", CANARY],
        check=True,
        timeout=5,
    )
    exposed = password.queryText().getText(0, -1)
    if CANARY in exposed:
        raise RuntimeError("AT-SPI exposed the API-key cleartext")
    if not exposed or set(exposed) == set(CANARY):
        raise RuntimeError("AT-SPI did not return a masked password representation")

    report = {
        "schema_version": 1,
        "platform": "linux",
        "backend": "at-spi",
        "application_exposed": True,
        "document_exposed": True,
        "primary_navigation_exposed": True,
        "required_controls_named": True,
        "required_controls_focusable": True,
        "unnamed_focusable_control_roles": [],
        "keyboard_focus_sequence": ["Goal", "Open folder", "Create task"],
        "simple_to_advanced_native_input": True,
        "credential_control_role": "password text",
        "credential_accessible_value_redacted": True,
        "node_count": len(walk(app)),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"Linux native accessibility qualification passed: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
