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
WORKSPACE = AUDIT / "race-workspace"
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
        if "AUDIT_WRITE_AFTER_RESEARCH" in text and not previous_tool:
            write_requested.set()
            arguments = json.dumps({"file_path": str(WORKSPACE / "mode-should-deny.txt"), "content": "audit fixture"})
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
config = AUDIT / "race.acl"
config.write_text(f'''default_model = "mock/a"
providers "mock" {{
  base_url = "http://127.0.0.1:{server.server_port}/v1"
  api_key = "test-only"
  models "a" {{}}
}}
''')
env = dict(os.environ)
env.update(A3S_HOME=str(AUDIT / "race-home"), A3S_CONFIG=str(config),
           A3S_CODE_LOG_DIR=str(AUDIT / "race-logs"), RUST_LOG="off")
stderr = (AUDIT / "race-stderr.log").open("w")
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
    assert "result" in request("session/set_mode", {"sessionId": session_id, "modeId": "yolo"})
    first = send("session/prompt", {"sessionId": session_id, "prompt": [{"type": "text", "text": "AUDIT_RESEARCH_HOLD"}], "_meta": {"a3sTurn": {"research": True}}})
    assert started.wait(15), "research turn did not reach fixture model"
    assert "result" in request("session/set_mode", {"sessionId": session_id, "modeId": "plan"})
    release.set()
    research_result = wait(first)
    visible = request("_x.ai/session/info", {"sessionId": session_id})
    assert not target.exists(), "fixture file existed before the write request"
    second = send("session/prompt", {"sessionId": session_id, "prompt": [{"type": "text", "text": "AUDIT_WRITE_AFTER_RESEARCH"}]})
    assert write_requested.wait(15), "fixture model did not request the write"
    second_response = wait(second)
    outcome = {"research_result": research_result, "reported_posture": visible["result"]["result"]["permissionPosture"],
               "write_executed_after_plan": (WORKSPACE / "mode-should-deny.txt").exists()}
    (AUDIT / "race-result.json").write_text(json.dumps(outcome, indent=2))
    print(json.dumps(outcome, indent=2))
    assert outcome["reported_posture"] == "plan"
    assert not outcome["write_executed_after_plan"], "Plan must survive research cleanup"
    assert "result" in request("session/set_mode", {"sessionId":session_id,"modeId":"yolo"})
    bound = request("session/prompt", {"sessionId":session_id,
        "prompt":[{"type":"text","text":"AUDIT_WRITE_AFTER_RESEARCH"}],
        "_meta":{"permissionPosture":"plan"}})
    assert not target.exists(), "prompt-bound Plan must narrow a prior Yolo mode"
    reported = request("_x.ai/session/info", {"sessionId":session_id})
    assert reported["result"]["result"]["permissionPosture"] == "plan"
    outcome["prompt_bound_plan_enforced"] = True
    (AUDIT / "race-result.json").write_text(json.dumps(outcome, indent=2))
finally:
    release.set()
    process.terminate()
    process.wait(timeout=10)
    server.shutdown()
    stderr.close()
