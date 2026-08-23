#!/usr/bin/env python3
"""Qualify the installed Linux desktop through the native AT-SPI bridge."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.request
from pathlib import Path
from typing import Any, Iterable

CANARY = "MCG_NATIVE_SECRET_CANARY"
REVIEWED_PROJECT_NAME = "native-reviewed-topology-project"
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
    parser.add_argument("--reviewed-topology-marker", type=Path)
    parser.add_argument("--host")
    parser.add_argument("--token-file", type=Path)
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


def wait_for_named_node(app: Any, names: set[str], deadline: float) -> Any:
    while time.monotonic() < deadline:
        for node in walk(app):
            if accessible_name(node) in names:
                return node
        time.sleep(0.1)
    expected = ", ".join(sorted(names))
    raise RuntimeError(f"AT-SPI did not expose a node named one of: {expected}")


def wait_for_combo_with_options(app: Any, options: set[str], deadline: float) -> Any:
    while time.monotonic() < deadline:
        for node in walk(app):
            if role_name(node) != "combo box":
                continue
            option_names = {
                accessible_name(candidate) for candidate in walk(node, budget=100)
            }
            if options.issubset(option_names):
                return node
        time.sleep(0.1)
    expected = ", ".join(sorted(options))
    raise RuntimeError(f"AT-SPI did not expose the combo box with options: {expected}")


def select_named_option(combo: Any, label: str, atspi: Any) -> None:
    options = [
        node
        for node in walk(combo, budget=100)
        if accessible_name(node) in {"Guided", "Balanced", "Autonomous"}
    ]
    candidates = [node for node in options if accessible_name(node) == label]
    if len(candidates) != 1:
        raise RuntimeError(f"AT-SPI did not expose exactly one {label!r} option")
    if os.environ.get("MCG_ATSPI_DEBUG") == "1":
        for node in [combo, *options]:
            actions: list[str] = []
            try:
                action = node.queryAction()
                actions = [action.getName(index) for index in range(action.nActions)]
            except Exception:
                pass
            selected = has_state(node, atspi.STATE_SELECTED)
            focused = has_state(node, atspi.STATE_FOCUSED)
            value: float | None = None
            try:
                value = node.queryValue().currentValue
            except Exception:
                pass
            print(
                "AT-SPI combo diagnostic: "
                f"role={role_name(node)!r} name={accessible_name(node)!r} "
                f"index={node.getIndexInParent()} selected={selected} "
                f"focused={focused} value={value!r} actions={actions!r}",
                file=sys.stderr,
                flush=True,
            )
    component = combo.queryComponent()
    if not component.scrollTo(atspi.SCROLL_ANYWHERE) or not component.grabFocus():
        raise RuntimeError("AT-SPI could not focus the autonomy combo box")
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        if has_state(combo, atspi.STATE_FOCUSED):
            break
        time.sleep(0.05)
    else:
        raise RuntimeError("AT-SPI focus did not reach the autonomy combo box")
    subprocess.run(
        [
            "xdotool",
            "key",
            "--clearmodifiers",
            "--delay",
            "250",
            "Alt+Down",
            label[0].casefold(),
            "Return",
            "Tab",
        ],
        check=True,
        timeout=5,
    )
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        selected_names = [
            accessible_name(candidate)
            for candidate in options
            if has_state(candidate, atspi.STATE_SELECTED)
        ]
        if label in accessible_name(combo) and selected_names == [label]:
            break
        time.sleep(0.05)
    else:
        raise RuntimeError(f"AT-SPI could not select the {label!r} option")
    if os.environ.get("MCG_ATSPI_DEBUG") == "1":
        print(
            "AT-SPI combo result: "
            f"name={accessible_name(combo)!r} selected={selected_names!r}",
            file=sys.stderr,
            flush=True,
        )


def wait_for_focused(app: Any, name: str, focused_state: Any, deadline: float) -> Any:
    while time.monotonic() < deadline:
        for node in walk(app):
            if accessible_name(node) == name and has_state(node, focused_state):
                return node
        time.sleep(0.05)
    focused = [
        f"{role_name(node)}:{accessible_name(node)}"
        for node in walk(app)
        if has_state(node, focused_state)
    ]
    raise RuntimeError(f"AT-SPI focus did not reach {name!r}; focused={focused!r}")


def wait_for_focused_prefix(
    app: Any, role: str, prefix: str, focused_state: Any, deadline: float
) -> Any:
    while time.monotonic() < deadline:
        for node in walk(app):
            if (
                role_name(node) == role
                and accessible_name(node).startswith(prefix)
                and has_state(node, focused_state)
            ):
                return node
        time.sleep(0.05)
    raise RuntimeError(f"AT-SPI focus did not reach {role!r} beginning with {prefix!r}")


def select_named_project(app: Any, name: str, atspi: Any, deadline: float) -> None:
    wait_for_named_node(app, {name}, deadline)
    while time.monotonic() < deadline:
        buttons = [
            node
            for node in walk(app)
            if role_name(node) == "push button"
            and accessible_name(node) in {"Use project", "Selected"}
        ]
        if len(buttons) == 1:
            if accessible_name(buttons[0]) == "Use project":
                click_accessible(buttons[0], atspi)
            return
        time.sleep(0.1)
    raise RuntimeError(
        f"AT-SPI did not expose exactly one selection control for project {name!r}"
    )


def wait_for_text_content(node: Any, fragments: set[str], deadline: float) -> None:
    while time.monotonic() < deadline:
        exposed: list[str] = []
        for candidate in walk(node, budget=500):
            name = accessible_name(candidate)
            if name:
                exposed.append(name)
            try:
                text = candidate.queryText().getText(0, -1)
                if text:
                    exposed.append(text)
            except Exception:
                pass
        rendered = " ".join(exposed)
        if fragments.issubset(set(rendered.split())):
            return
        time.sleep(0.1)
    expected = ", ".join(sorted(fragments))
    raise RuntimeError(f"AT-SPI did not expose the required text fragments: {expected}")


def click_accessible(node: Any, atspi: Any) -> None:
    component = node.queryComponent()
    if not component.scrollTo(atspi.SCROLL_ANYWHERE):
        raise RuntimeError(
            f"AT-SPI could not scroll {accessible_name(node)!r} into view"
        )
    time.sleep(0.1)
    box = component.getExtents(atspi.DESKTOP_COORDS)
    if box.width < 2 or box.height < 2:
        raise RuntimeError(
            f"AT-SPI exposed invalid bounds for {accessible_name(node)!r}"
        )
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


def activate_accessible(node: Any, atspi: Any) -> None:
    try:
        action = node.queryAction()
        if action.nActions > 0 and action.doAction(0):
            return
    except Exception:
        pass
    click_accessible(node, atspi)


def read_settings(host: str, token: str) -> dict[str, Any]:
    request = urllib.request.Request(
        f"{host.rstrip('/')}/api/v1/settings",
        headers={"Authorization": f"Bearer {token}"},
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        return json.load(response)


def wait_for_guided_settings(
    host: str, token: str, previous_revision: int, deadline: float
) -> dict[str, Any]:
    while time.monotonic() < deadline:
        settings = read_settings(host, token)
        if (
            settings["revision"] > previous_revision
            and settings["autonomy"] == "guided"
        ):
            return settings
        time.sleep(0.1)
    raise RuntimeError("The native settings save did not persist Guided autonomy")


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
        candidates = []
        for candidate in children(desktop):
            if accessible_name(candidate).casefold() != args.application.casefold():
                continue
            try:
                if has_state(candidate, pyatspi.STATE_DEFUNCT):
                    continue
            except Exception:
                continue
            candidates.append(candidate)
        if candidates:
            app = candidates[-1]
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
        raise RuntimeError(
            f"AT-SPI is missing required native controls: {', '.join(missing)}"
        )

    focusable_required = [
        "goal",
        "open_folder",
        "create_task",
        "openai_oauth",
        "add_provider",
    ]
    not_focusable = [
        name
        for name in focusable_required
        if not has_state(required[name], pyatspi.STATE_FOCUSABLE)
    ]
    if not_focusable:
        raise RuntimeError(
            f"Required controls are not keyboard-focusable: {', '.join(not_focusable)}"
        )

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
            "AT-SPI exposed unnamed focusable controls for roles: "
            + ", ".join(unnamed_focusable)
        )

    goal = required["goal"]
    assert goal is not None
    goal.queryComponent().scrollTo(pyatspi.SCROLL_ANYWHERE)
    if not goal.queryComponent().grabFocus():
        raise RuntimeError("AT-SPI could not focus the Goal control")
    wait_for_focused(
        app, "Goal", pyatspi.STATE_FOCUSED, time.monotonic() + args.timeout_seconds
    )
    subprocess.run(["xdotool", "key", "Tab"], check=True, timeout=5)
    if args.reviewed_topology_marker is not None:
        wait_for_focused_prefix(
            app,
            "combo box",
            "Project ",
            pyatspi.STATE_FOCUSED,
            time.monotonic() + args.timeout_seconds,
        )
        subprocess.run(["xdotool", "key", "Tab"], check=True, timeout=5)
    wait_for_focused(
        app,
        "Open folder",
        pyatspi.STATE_FOCUSED,
        time.monotonic() + args.timeout_seconds,
    )
    subprocess.run(["xdotool", "key", "Tab"], check=True, timeout=5)
    wait_for_focused(
        app,
        "Create task",
        pyatspi.STATE_FOCUSED,
        time.monotonic() + args.timeout_seconds,
    )

    mode = wait_for_node(
        app,
        "toggle button",
        {"Advanced mode"},
        time.monotonic() + args.timeout_seconds,
    )
    if not has_state(mode, pyatspi.STATE_FOCUSABLE):
        raise RuntimeError("The mode switch is not keyboard-focusable through AT-SPI")
    click_accessible(mode, pyatspi)
    wait_for_node(
        app,
        "toggle button",
        {"Simple mode"},
        time.monotonic() + args.timeout_seconds,
    )

    add_provider = wait_for_node(
        app,
        "push button",
        {"Add API or local provider"},
        time.monotonic() + args.timeout_seconds,
    )
    click_accessible(add_provider, pyatspi)
    password = wait_for_node(
        app,
        "password text",
        {"API key"},
        time.monotonic() + args.timeout_seconds,
    )
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

    reviewed_topology_qualified = False
    if args.reviewed_topology_marker is not None:
        if not args.host or args.token_file is None:
            raise RuntimeError("Reviewed-topology UAT requires --host and --token-file")
        token = args.token_file.read_text(encoding="utf-8").strip()
        if not token:
            raise RuntimeError("The installed loopback token is empty")
        marker = args.reviewed_topology_marker.resolve()
        if marker.exists():
            raise RuntimeError(
                "Reviewed-topology marker already exists before native interaction"
            )
        settings = wait_for_node(
            app, "link", {"Settings"}, time.monotonic() + args.timeout_seconds
        )
        click_accessible(settings, pyatspi)
        autonomy = wait_for_combo_with_options(
            app,
            {"Guided", "Balanced", "Autonomous"},
            time.monotonic() + args.timeout_seconds,
        )
        select_named_option(autonomy, "Guided", pyatspi)
        save_settings = wait_for_node(
            app,
            "push button",
            {"Save settings"},
            time.monotonic() + args.timeout_seconds,
        )
        settings_revision = read_settings(args.host, token)["revision"]
        activate_accessible(save_settings, pyatspi)
        wait_for_guided_settings(
            args.host,
            token,
            settings_revision,
            time.monotonic() + args.timeout_seconds,
        )
        saved_autonomy = wait_for_combo_with_options(
            app,
            {"Guided", "Balanced", "Autonomous"},
            time.monotonic() + args.timeout_seconds,
        )
        if "Guided" not in accessible_name(saved_autonomy):
            raise RuntimeError(
                "The native settings save did not persist Guided autonomy"
            )
        projects = wait_for_node(
            app, "link", {"Projects"}, time.monotonic() + args.timeout_seconds
        )
        click_accessible(projects, pyatspi)
        select_named_project(
            app,
            REVIEWED_PROJECT_NAME,
            pyatspi,
            time.monotonic() + args.timeout_seconds,
        )
        topology = wait_for_named_node(
            app,
            {"Project topology plan"},
            time.monotonic() + args.timeout_seconds,
        )
        wait_for_text_content(
            topology,
            {"Stage", "1", "database", "2", "api", "3", "web"},
            time.monotonic() + args.timeout_seconds,
        )
        reason = wait_for_named_node(
            app, {"Reviewed topology reason"}, time.monotonic() + args.timeout_seconds
        )
        click_accessible(reason, pyatspi)
        subprocess.run(["xdotool", "key", "ctrl+a"], check=True, timeout=5)
        subprocess.run(
            [
                "xdotool",
                "type",
                "--clearmodifiers",
                "Run the installed native reviewed project tests",
            ],
            check=True,
            timeout=5,
        )
        review = wait_for_node(
            app,
            "push button",
            {"Review this plan"},
            time.monotonic() + args.timeout_seconds,
        )
        click_accessible(review, pyatspi)
        run = wait_for_node(
            app,
            "push button",
            {"Run reviewed plan"},
            time.monotonic() + args.timeout_seconds,
        )
        click_accessible(run, pyatspi)
        approve = wait_for_node(
            app,
            "push button",
            {"Approve and run"},
            time.monotonic() + args.timeout_seconds,
        )
        if marker.exists():
            raise RuntimeError("Reviewed command executed before native approval")
        click_accessible(approve, pyatspi)
        reviewed_plans = wait_for_named_node(
            app,
            {"Reviewed topology plans"},
            time.monotonic() + args.timeout_seconds,
        )
        wait_for_text_content(
            reviewed_plans,
            {"succeeded"},
            time.monotonic() + args.timeout_seconds,
        )
        deadline = time.monotonic() + args.timeout_seconds
        while time.monotonic() < deadline:
            if marker.is_file() and marker.read_text(encoding="utf-8") == "x":
                reviewed_topology_qualified = True
                break
            time.sleep(0.1)
        if not reviewed_topology_qualified:
            raise RuntimeError("Reviewed native command did not execute exactly once")

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
        "keyboard_focus_sequence": (
            ["Goal", "Project", "Open folder", "Create task"]
            if args.reviewed_topology_marker is not None
            else ["Goal", "Open folder", "Create task"]
        ),
        "simple_to_advanced_native_input": True,
        "credential_control_role": "password text",
        "credential_accessible_value_redacted": True,
        "reviewed_topology_native_ui": reviewed_topology_qualified,
        "reviewed_topology_waited_for_approval": reviewed_topology_qualified,
        "reviewed_topology_terminal_succeeded_visible": reviewed_topology_qualified,
        "node_count": len(walk(app)),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(report, separators=(",", ":")) + "\n", encoding="utf-8"
    )
    print(f"Linux native accessibility qualification passed: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
