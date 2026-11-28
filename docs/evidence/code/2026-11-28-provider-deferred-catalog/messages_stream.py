#!/usr/bin/env python3
"""Run frozen ACP cases through Aster's Messages adapter and record actual SSE usage."""

import argparse
import asyncio
from collections import Counter
from contextlib import closing
from http.client import HTTPSConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import sys
from threading import Lock, Thread

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "2026-11-27-model-selected-tool-catalog"))
import full_catalog
import pilot

base = pilot.base


class MessagesRelay(base.Relay):
    def __init__(self, credential, logfile):
        self.active = None
        self.lock = Lock()
        self.rows = []
        self.logfile = logfile
        self.started = Counter()
        self.pending = Counter()
        relay = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                if self.path != "/v1/messages":
                    self.send_error(404)
                    return
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                with relay.lock:
                    binding = relay.active
                    if binding is not None:
                        relay.started[binding[0]] += 1
                        relay.pending[binding[0]] += 1
                if binding is None:
                    self.send_error(409)
                    return
                self.forward(request, binding)

            def forward(self, request, binding):
                request.update(thinking={"type": "disabled"}, temperature=0, max_tokens=1024)
                request.pop("reasoning_effort", None)
                row = {
                    "status": None, "models": [], "usage": [], "tool_calls": [],
                    "tool_definitions": len(request.get("tools", [])),
                    "tool_result_groups": [
                        sum(block.get("type") == "tool_result" for block in message["content"])
                        for message in request["messages"] if isinstance(message["content"], list)
                    ],
                    "stream": bool(request.get("stream")),
                }
                usage = {}

                def observe(packet):
                    kind = packet.get("type")
                    if kind == "message_start":
                        observe(packet["message"])
                    elif kind == "message":
                        row["models"] = [packet["model"]]
                        usage.update(packet.get("usage", {}))
                        row["tool_calls"].extend(
                            block["name"] for block in packet.get("content", [])
                            if block["type"] == "tool_use"
                        )
                    elif kind == "message_delta":
                        usage.update(packet.get("usage", {}))
                    elif kind == "content_block_start" and packet["content_block"]["type"] == "tool_use":
                        row["tool_calls"].append(packet["content_block"]["name"])

                try:
                    with closing(HTTPSConnection("api.deepseek.com", timeout=60)) as connection:
                        connection.request(
                            "POST", "/anthropic/v1/messages",
                            json.dumps(request, ensure_ascii=False).encode(),
                            {"Authorization": "Bearer " + credential, "Content-Type": "application/json",
                             "anthropic-version": "2023-06-01"},
                        )
                        response = connection.getresponse()
                        row["status"] = response.status
                        content_type = response.getheader("Content-Type", "application/json")
                        self.send_response(response.status)
                        self.send_header("Content-Type", content_type)
                        self.end_headers()
                        if response.status == 200 and "text/event-stream" in content_type:
                            while line := response.readline():
                                if line.startswith(b"data:"):
                                    observe(json.loads(line[5:]))
                                self.wfile.write(line)
                                self.wfile.flush()
                        else:
                            data = response.read()
                            packet = json.loads(data)
                            if response.status == 200:
                                observe(packet)
                            else:
                                row["error"] = packet.get("error")
                            self.wfile.write(data)
                finally:
                    if "output_tokens" in usage and "input_tokens" in usage:
                        row["usage"] = [pilot.chat_usage(usage, True)]
                    relay.record(row, binding, completed=True)

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        Thread(target=self.server.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.server.server_port}"


def run(args):
    original_process = base.acp_process

    def messages_process(arm, binary, directory, *remaining, **options):
        command, env = original_process(arm, binary, directory, *remaining, **options)
        config = Path(env["ASTER_CONFIG"])
        text = config.read_text(encoding="utf-8")
        config.write_text(
            text.replace('providers "deepseek" {', 'providers "deepseek" {\n  protocol = "anthropic"'),
            encoding="utf-8",
        )
        return command, env

    base.acp_process = messages_process
    base.Relay = MessagesRelay
    if args.scenario == "short":
        args.release = args.acp
        args.order = ["release"]
        asyncio.run(base.evaluate(args))
    else:
        args.reference_acp = args.acp
        args.native_catalog = False
        args.default_catalog = True
        args.annotate_signatures = False
        args.native_required = False
        args.ungated = True
        args.disk_content_attestation = args.scenario == "verified_edit_exact_count"
        asyncio.run(full_catalog.evaluate(args))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenario", required=True, choices=("short", "batch_mixed", "verified_edit_exact_count"))
    for name in ("acp", "embedding", "out"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    run(parser.parse_args())
