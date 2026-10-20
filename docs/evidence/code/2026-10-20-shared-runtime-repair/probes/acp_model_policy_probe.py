import http.server
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time

ROOT = Path(subprocess.check_output(['git','rev-parse','--show-toplevel'],text=True).strip())
AUDIT = Path(os.environ['ASTER_REPAIR_PROBE_DIR']).resolve()
AUDIT.mkdir(parents=True, exist_ok=True)
WORKSPACE = AUDIT / "model-policy-workspace"
WORKSPACE.mkdir(exist_ok=True)
target = WORKSPACE / "mode-should-deny.txt"
target.unlink(missing_ok=True)
started = threading.Event()
release = threading.Event()
write_requested = threading.Event()

class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        latest = next((m for m in reversed(body.get("messages", [])) if m.get("role") == "user"), {})
        content = latest.get("content", "")
        text = content if isinstance(content, str) else "".join(p.get("text", "") for p in content)
        if "AUDIT_RESEARCH_HOLD" in text and not release.is_set():
            started.set()
            assert release.wait(20), "test did not release held model request"
        messages = body.get("messages", [])
        latest_index = max(i for i,m in enumerate(messages) if m.get("role") == "user")
        previous_tool = any(m.get("role") == "tool" for m in messages[latest_index+1:])
        if "AUDIT_WRITE_MODEL" in text and not previous_tool:
            write_requested.set()
            arguments = json.dumps({"file_path": str(WORKSPACE / ("before.txt" if "BEFORE" in text else "after.txt" if "AFTER" in text else "after-mode.txt")), "content": "audit fixture"})
            delta = {"tool_calls": [{"index": 0, "id": "audit-write", "function": {"name": "write", "arguments": arguments}}]}
            reason = "tool_calls"
        else:
            delta, reason = {"content": "fixture complete"}, "stop"
        payload = "data: " + json.dumps({"choices": [{"delta": delta}]}) + "\n\n"
        payload += "data: " + json.dumps({"choices": [{"delta": {}, "finish_reason": reason}]}) + "\n\ndata: [DONE]\n\n"
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(payload.encode())))
        self.end_headers()
        self.wfile.write(payload.encode())

server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Model)
threading.Thread(target=server.serve_forever, daemon=True).start()
config = AUDIT / "model-policy.acl"
config.write_text(f'''default_model = "mock/a"
providers "mock" {{
  base_url = "http://127.0.0.1:{server.server_port}/v1"
  api_key = "test-only"
  models "a" {{}}
  models "b" {{}}
}}
''')
env = dict(os.environ)
env.update(A3S_HOME=str(AUDIT / "model-policy-home"), A3S_CONFIG=str(config),
           A3S_CODE_LOG_DIR=str(AUDIT / "model-policy-logs"), RUST_LOG="off")
stderr = (AUDIT / "model-policy-stderr.log").open("w")
process = subprocess.Popen([str(ROOT / "target/debug/a3s-code-acp"), "--config", str(config), "--cwd", str(WORKSPACE)],
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr, env=env, text=True)
lines = queue.Queue()
def read():
    for line in process.stdout:
        lines.put(json.loads(line))
threading.Thread(target=read, daemon=True).start()
counter = 0
def send(method, params):
    global counter
    counter += 1
    process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": counter, "method": method, "params": params}) + "\n")
    process.stdin.flush()
    return counter
def wait(identity):
    while True:
        value = lines.get(timeout=30)
        if value.get("method") == "session/request_permission":
            process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": value["id"], "result": {"outcome": {"outcome": "cancelled"}}}) + "\n")
            process.stdin.flush()
        if value.get("id") == identity:
            return value
def request(method, params):
    return wait(send(method, params))
try:
    assert "result" in request("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
    opened = request("session/new", {"cwd": str(WORKSPACE), "mcpServers": []})
    session_id = opened["result"]["sessionId"]
    for name in ["before.txt", "after.txt", "after-mode.txt"]:
        (WORKSPACE/name).unlink(missing_ok=True)
    asks = [0]
    old_wait = wait
    def wait(identity):
        while True:
            value = lines.get(timeout=30)
            if value.get("method") == "session/request_permission":
                asks[0] += 1
                process.stdin.write(json.dumps({"jsonrpc":"2.0","id":value["id"],"result":{"outcome":{"outcome":"cancelled"}}})+"\n")
                process.stdin.flush()
            if value.get("id") == identity:
                return value
    assert "result" in request("_x.ai/permission_posture", {"sessionId":session_id,"posture":"yolo","deny":["write"]})
    def prompt(text):
        return request("session/prompt", {"sessionId":session_id,"prompt":[{"type":"text","text":text}]})
    before_response = prompt("AUDIT_WRITE_MODEL_BEFORE")
    before = {"file_created": (WORKSPACE/"before.txt").exists(), "permission_asks": asks[0]}
    assert before == {"file_created":False,"permission_asks":0}, before
    assert "result" in request("session/set_model", {"sessionId":session_id,"modelId":"mock/b"})
    after_response = prompt("AUDIT_WRITE_MODEL_AFTER")
    after = {"file_created": (WORKSPACE/"after.txt").exists(), "permission_asks": asks[0]}
    assert "result" in request("session/set_mode", {"sessionId":session_id,"modeId":"yolo"})
    mode_response = prompt("AUDIT_WRITE_MODEL_CHANGED")
    mode = {"file_created": (WORKSPACE/"after-mode.txt").exists(), "permission_asks": asks[0]-after["permission_asks"]}
    result = {"before_model":before,"after_model":after,"after_reselecting_yolo":mode}
    (AUDIT/"model-policy-result.json").write_text(json.dumps(result,indent=2))
    print(json.dumps(result,indent=2))
    assert after == {"file_created":False,"permission_asks":0}, after
    assert mode == {"file_created":False,"permission_asks":0}, mode

finally:
    release.set()
    process.terminate()
    process.wait(timeout=10)
    server.shutdown()
    stderr.close()
