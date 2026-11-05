#!/usr/bin/env python3
"""WIKI-01: scoped Wiki defaults to Analysis, switches tabs and closes."""

from __future__ import annotations

import argparse
import json
import time
import traceback
from pathlib import Path
from typing import Any

from core_navigation import AgentDesktop, wait_find

TAB_NAMES = ("分析", "百科", "产物")


def tabs(driver: AgentDesktop) -> list[dict[str, Any]]:
    snapshot = driver.call("snapshot", "--app", driver.app, "--window-id", driver.window_id, "-i", "--compact")
    tree = (snapshot.get("data") or {}).get("tree") or {}
    found: list[dict[str, Any]] = []

    def visit(node: Any) -> None:
        if isinstance(node, dict):
            if node.get("role") == "radiobutton" and node.get("name") in TAB_NAMES:
                found.append(node)
            for child in node.get("children") or []:
                visit(child)
        elif isinstance(node, list):
            for child in node:
                visit(child)

    visit(tree)
    return found


def wait_tabs(driver: AgentDesktop) -> list[dict[str, Any]]:
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        current = tabs(driver)
        if current:
            return current
        time.sleep(0.15)
    raise AssertionError("knowledge tabs did not appear")


def wait_knowledge_closed(driver: AgentDesktop) -> None:
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        if not tabs(driver):
            return
        time.sleep(0.15)
    raise AssertionError("knowledge view did not close")


def run(driver: AgentDesktop) -> dict[str, Any]:
    driver.bind_window()
    if not tabs(driver):
        opener = wait_find(driver, "button", "查看任务", exact=False)
        driver.call("focus", opener["ref_id"])
        driver.press("return")
    initial = wait_tabs(driver)
    names = [tab.get("name") for tab in initial]
    if names != list(TAB_NAMES):
        raise AssertionError(f"expected 分析/百科/产物 order, got {names}")
    selected = [tab.get("name") for tab in initial if tab.get("value") == "true" or "selected" in (tab.get("states") or [])]
    if selected != ["分析"]:
        raise AssertionError(f"expected Analysis by default, got {selected}")
    visited = []
    for name in ("百科", "产物"):
        driver.click("radiobutton", name)
        current = wait_find(driver, "radiobutton", name)
        if current.get("value") != "true" and "selected" not in (current.get("states") or []):
            raise AssertionError(f"tab {name} did not become selected")
        visited.append(name)
    driver.click("button", "关闭")
    wait_knowledge_closed(driver)
    return {"default": "分析", "order": names, "visited": visited, "closed": True}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True)
    parser.add_argument("--pid", required=True, type=int)
    parser.add_argument("--window-id", required=True)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    driver = AgentDesktop(args.app, args.pid, args.window_id)
    evidence: dict[str, Any] = {"case": "WIKI-01", "status": "failed", "steps": driver.steps}
    try:
        evidence["details"] = run(driver)
        evidence["status"] = "passed"
    except Exception as error:
        evidence["error"] = str(error)
        evidence["traceback"] = traceback.format_exc()
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(evidence, ensure_ascii=False, indent=2))
    return 0 if evidence["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
