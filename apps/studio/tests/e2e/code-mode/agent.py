#!/usr/bin/env python3
"""Local ACP peer for the code-mode window gate.

Speaks newline JSON-RPC. A prompt that contains ``code-mode-e2e-write`` writes
one file in the session cwd and reports that write. Every other prompt ends
the turn with no tool and no network call.
"""

from __future__ import annotations

import json
import os
import sys

WRITE_MARKER = "code-mode-e2e-write"
FILE_NAME = "e2e-note.txt"
FILE_BODY = "e2e-note-line\n"
SESSION_ID = "code-mode-e2e"


def send(message: dict) -> None:
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def result(message_id, payload: dict) -> None:
    send({"jsonrpc": "2.0", "id": message_id, "result": payload})


def update(payload: dict) -> None:
    send(
        {
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {"sessionId": SESSION_ID, "update": payload},
        }
    )


def prompt_text(params: dict) -> str:
    prompt = params.get("prompt")
    if isinstance(prompt, str):
        return prompt
    chunks: list[str] = []
    if isinstance(prompt, list):
        for item in prompt:
            if isinstance(item, str):
                chunks.append(item)
            elif isinstance(item, dict):
                text = item.get("text")
                if isinstance(text, str):
                    chunks.append(text)
                content = item.get("content")
                if isinstance(content, dict) and isinstance(content.get("text"), str):
                    chunks.append(content["text"])
    return "\n".join(chunks)


def write_note(cwd: str) -> str:
    directory = cwd if cwd and os.path.isdir(cwd) else os.getcwd()
    path = os.path.join(directory, FILE_NAME)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(FILE_BODY)
    receipt = os.environ.get("CODE_MODE_WRITE_RESULT")
    if receipt:
        with open(receipt, "w", encoding="utf-8") as handle:
            handle.write(path)
    return path


def report_write(message_id, path: str) -> None:
    update(
        {
            "sessionUpdate": "tool_call",
            "toolCallId": "e2e-write",
            "title": "write",
            "status": "in_progress",
            "rawInput": {"path": path, "content": FILE_BODY},
        }
    )
    update(
        {
            "sessionUpdate": "tool_call_update",
            "toolCallId": "e2e-write",
            "status": "completed",
            "rawOutput": {"path": path, "content": FILE_BODY},
            "content": [
                {
                    "type": "diff",
                    "path": path,
                    "oldText": "",
                    "newText": FILE_BODY.rstrip("\n"),
                }
            ],
        }
    )
    result(message_id, {"stopReason": "end_turn"})


def handle(message: dict, cwd: str) -> str:
    method = message.get("method")
    message_id = message.get("id")
    params = message.get("params") if isinstance(message.get("params"), dict) else {}
    if method == "initialize" and message_id is not None:
        result(message_id, {"protocolVersion": 1})
        return cwd
    if method == "session/new" and message_id is not None:
        next_cwd = params.get("cwd") if isinstance(params.get("cwd"), str) else cwd
        result(message_id, {"sessionId": SESSION_ID})
        return next_cwd or cwd
    if method == "session/set_mode" and message_id is not None:
        result(message_id, {})
        return cwd
    if method == "session/prompt" and message_id is not None:
        if WRITE_MARKER in prompt_text(params):
            report_write(message_id, write_note(cwd))
        else:
            result(message_id, {"stopReason": "end_turn"})
        return cwd
    if method == "session/cancel":
        return cwd
    if message_id is not None:
        result(message_id, {})
    return cwd


def main() -> None:
    cwd = ""
    for line in sys.stdin:
        text = line.strip()
        if not text:
            continue
        try:
            message = json.loads(text)
        except json.JSONDecodeError:
            continue
        if isinstance(message, dict):
            cwd = handle(message, cwd)


if __name__ == "__main__":
    main()
