#!/usr/bin/env python3
"""Install a final macOS archive and exercise its CLI/ACP sandbox without external assets."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import secrets
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[1]
def housekeeping(body):
    raw = json.dumps(body, ensure_ascii=False)
    if 'emit_pre_analysis' in raw or 'You are a pre-analysis assistant' in raw:
        result = {'intent': 'GeneralPurpose', 'requires_planning': False,
                  'goal': {'description': 'Collect synthetic form answers', 'success_criteria': []},
                  'execution_plan': {'complexity': 'Simple', 'steps': [], 'required_tools': []},
                  'optimized_input': ''}
        return {'role': 'assistant', 'content': json.dumps(result)}, 'stop', {'kind': 'pre_analysis'}
    if 'You extract durable, reusable memory' in raw:
        return {'role': 'assistant', 'content': '{"items":[]}'}, 'stop', {'kind': 'memory'}
    raise ValueError("unexpected housekeeping request")

COMMAND = "printf 'ASTER_RELEASE_SANDBOX_OK\\n'; uname -s; cat release-input.txt"


class ProbeHandler(BaseHTTPRequestHandler):
    model_name = "release-sandbox"

    def decide(self, body):
        raw = json.dumps(body)
        if any(text in raw for text in (
            "emit_pre_analysis", "You are a pre-analysis assistant", "You extract durable, reusable memory",
        )):
            return housekeeping(body)
        messages = body.get("messages", [])
        calls = {call["id"] for message in messages for call in message.get("tool_calls", [])
                 if call.get("function", {}).get("name") == "bash"}
        receipts = [message.get("content", "") for message in messages
                    if message.get("role") == "tool" and message.get("tool_call_id") in calls]
        if receipts:
            return ({"role": "assistant", "content": "RELEASE_PROBE_RECEIVED"}, "stop",
                    {"kind": "tool_result", "output": receipts[-1]})
        if calls or self.server.attempted:
            return ({"role": "assistant", "content": "RELEASE_PROBE_MISSING_RECEIPT"}, "stop",
                    {"kind": "missing_receipt"})
        self.server.attempted = True
        call = {"id": "release-sandbox", "type": "function", "function": {
            "name": "bash", "arguments": json.dumps({"command": COMMAND})}}
        return ({"role": "assistant", "content": None, "tool_calls": [call]}, "tool_calls",
                {"kind": "tool_call", "command": COMMAND})


    def log_message(self, *args):
        pass

    def send(self, payload, content_type='application/json'):
        data = payload.encode()
        self.send_response(200)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.send(json.dumps({'object': 'list', 'data': [{'id': self.model_name, 'object': 'model'}]}))

    def do_POST(self):
        try:
            length = int(self.headers.get('Content-Length', '0'))
            if not 0 < length <= 2_000_000:
                self.send_error(413)
                return
            body = json.loads(self.rfile.read(length))
            if not isinstance(body, dict):
                raise ValueError('body must be an object')
        except (ValueError, TypeError):
            self.send_error(400)
            return
        if self.path == '/v1/systemone':
            self.send(json.dumps({'model': 'apofasi-lexical-fixture',
                                 'answers': {'surface': {'type': 'choice', 'choice': 'prose'}}}))
            return
        try:
            message, finish, event = self.decide(body)
        except ValueError as error:
            self.send_error(422, str(error))
            return
        # Retain the synthetic receipt only, never request headers or complete model context.
        with self.server.events.open('a') as output:
            output.write(json.dumps(event, ensure_ascii=False) + '\n')
        if body.get('stream'):
            delta = {k: v for k, v in message.items() if k != 'role'}
            for index, call in enumerate(delta.get('tool_calls', [])):
                call['index'] = index
            chunks = [{'choices': [{'index': 0, 'delta': delta}]},
                      {'choices': [{'index': 0, 'delta': {}, 'finish_reason': finish}]}]
            self.send(''.join('data: ' + json.dumps(chunk) + '\n\n' for chunk in chunks)
                      + 'data: [DONE]\n\n', 'text/event-stream')
        else:
            self.send(json.dumps({'id': self.model_name, 'object': 'chat.completion',
                                 'choices': [{'index': 0, 'message': message, 'finish_reason': finish}],
                                 'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}))

def execution_passed(exit_code, events, nonce):
    receipts = [event["output"] for event in events if event["kind"] == "tool_result"]
    return (exit_code == 0 and sum(event["kind"] == "tool_call" for event in events) == 1
            and len(receipts) == 1 and isinstance(receipts[0], str)
            and receipts[0].startswith("Tool exit code: 0\n")
            and f"ASTER_RELEASE_SANDBOX_OK\nLinux\n{nonce}\n" in receipts[0])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    match = re.fullmatch(r"aster-cli-v([0-9]+\.[0-9]+\.[0-9]+)-macos\.tar\.gz", args.archive.name)
    if not match:
        parser.error("archive must be an aster-cli macOS release archive")
    if sys.platform != "darwin" or platform.machine() not in ("arm64", "aarch64"):
        parser.error("release sandbox qualification requires macOS ARM64 with working HVF")
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256()
    with args.archive.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    result = {"passed": False, "archive": args.archive.name, "sha256": digest.hexdigest()}
    try:
        with tempfile.TemporaryDirectory(prefix="aster-release-qualification-") as temporary:
            root = Path(temporary)
            assets, pair, home, workspace = (root / name for name in ("assets", "bin", "home", "workspace"))
            for path in (assets, home, workspace):
                path.mkdir()
            shutil.copy2(args.archive, assets / args.archive.name)
            version = match[1]
            (assets / "latest.json").write_text(json.dumps({"version": version, "tag": "aster-cli-v" + version,
                "assets": {"macos": {"name": args.archive.name, "sha256": result["sha256"]}}}, separators=(",", ":")))
            env = dict(os.environ, HOME=str(home), A3S_HOME=str(home / ".a3s"), GROK_HOME=str(home / ".a3s"),
                       ASTER_RELEASE_BASE=str(assets), A3S_DISABLE_CC_SWITCH="1", A3S_DISABLE_GROK_ACCOUNT="1",
                       A3S_DISABLE_CURSOR_ACCOUNT="1", A3S_ACP_AGENT_BIN=str(pair / "a3s-code-acp"))
            for key in ("ASTER_SANDBOX_DIR", "A3S_CODE_GUEST_SYSTEM_IMAGE", "A3S_CODE_KRUN_SHIM",
                        "ASTER_INSTALL_URL", "GROK_INSTALL_URL"):
                env.pop(key, None)
            with (output / "install.log").open("w") as log:
                subprocess.run(["bash", str(ROOT / "scripts/install.sh"), "--prefix", str(pair)],
                               env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
            nonce = secrets.token_hex(16)
            (workspace / "release-input.txt").write_text(nonce + "\n")
            server = ThreadingHTTPServer(("127.0.0.1", 0), ProbeHandler)
            server.events = output / "model-events.jsonl"
            server.events.write_text("")
            server.attempted = False
            thread = threading.Thread(target=server.serve_forever)
            thread.start()
            try:
                config = root / "host.acl"
                config.write_text('default_model = "fixture/release-sandbox"\nproviders "fixture" {\n'
                                  ' api_key = "synthetic-release-fixture"\n'
                                  f' base_url = "http://127.0.0.1:{server.server_port}/v1"\n'
                                  ' models "release-sandbox" {}\n}\n')
                env.update(A3S_CONFIG=str(config), A3S_CONFIG_FILE=str(config))
                with (output / "terminal.log").open("w") as log:
                    process = subprocess.Popen([str(pair / "aster"), "--single", "RELEASE-SANDBOX-PROBE",
                        "--no-plan", "--no-auto-update", "--yolo", "--max-turns", "3"],
                        cwd=workspace, env=env, stdin=subprocess.DEVNULL, stdout=log,
                        stderr=subprocess.STDOUT, start_new_session=True)
                    try:
                        process.wait(timeout=150)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGTERM)
                        try:
                            process.wait(timeout=10)
                        except subprocess.TimeoutExpired:
                            os.killpg(process.pid, signal.SIGKILL)
                            process.wait()
                        raise
                events = [json.loads(line) for line in server.events.read_text().splitlines()]
                result.update(passed=execution_passed(process.returncode, events, nonce),
                              exit_code=process.returncode, events=events)
            finally:
                server.shutdown()
                server.server_close()
                thread.join()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        result["error"] = str(error)
    (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
