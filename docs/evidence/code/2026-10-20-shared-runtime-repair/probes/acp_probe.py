import json
import os
from pathlib import Path
import queue
import subprocess
import threading

ROOT = Path(subprocess.check_output(['git','rev-parse','--show-toplevel'],text=True).strip())
AUDIT = Path(os.environ['ASTER_REPAIR_PROBE_DIR']).resolve()
AUDIT.mkdir(parents=True, exist_ok=True)
WORKSPACE = AUDIT / "acp-workspace"
WORKSPACE.mkdir(exist_ok=True)
CONFIG = AUDIT / "probe.acl"
CONFIG.write_text('''default_model = "mock/a"
providers "mock" {
  base_url = "http://127.0.0.1:9/v1"
  api_key = "test-only"
  models "a" {}
  models "b" {}
}
''')
env = dict(os.environ)
env.update(A3S_HOME=str(AUDIT / "agent-home"), A3S_CONFIG=str(CONFIG),
           A3S_CODE_LOG_DIR=str(AUDIT / "agent-logs"), RUST_LOG="off")
lines = queue.Queue()
stderr = (AUDIT / "acp-stderr.log").open("w")
process = subprocess.Popen(
    [str(ROOT / "target/debug/a3s-code-acp"), "--cwd", str(WORKSPACE), "--config", str(CONFIG)],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr, text=True, env=env,
)

def read():
    for line in process.stdout:
        lines.put(json.loads(line))
    lines.put(None)

threading.Thread(target=read, daemon=True).start()
next_id = 0

def request(method, params):
    global next_id
    next_id += 1
    identity = next_id
    process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": identity, "method": method, "params": params}) + "\n")
    process.stdin.flush()
    while True:
        value = lines.get(timeout=30)
        assert value is not None, "agent exited"
        if value.get("id") == identity:
            return value

try:
    init = request("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
    assert "result" in init, init
    opened = request("session/new", {"cwd": str(WORKSPACE), "mcpServers": []})
    assert "result" in opened, opened
    session_id = opened["result"]["sessionId"]
    plan = request("session/set_mode", {"sessionId": session_id, "modeId": "plan"})
    assert "result" in plan, plan
    unknown = request("session/set_mode", {"sessionId": session_id, "modeId": "misspelled-mode"})
    assert "error" in unknown, "unknown mode must be refused"
    posture = request("_x.ai/permission_posture", {"sessionId": session_id, "posture": "plan", "deny": ["write"]})
    assert "result" in posture, posture
    before = request("_x.ai/session/info", {"sessionId": session_id})
    model = request("session/set_model", {"sessionId": session_id, "modelId": "mock/b"})
    assert "result" in model, model
    after = request("_x.ai/session/info", {"sessionId": session_id})
    ghost = request("session/set_model", {"sessionId": "audit-session-never-created", "modelId": "mock/a"})
    assert "error" in ghost, "unknown session must be refused"
    ghost_info = request("_x.ai/session/info", {"sessionId": "audit-session-never-created"})
    seeded = request("session/new", {"cwd": str(WORKSPACE), "mcpServers": [], "_meta": {"yoloMode": True, "autoMode": False}})
    seeded_info = request("_x.ai/session/info", {"sessionId": seeded["result"]["sessionId"]})
    assert after["result"]["result"]["permissionPosture"] == "plan"
    assert ghost_info["result"]["result"] is None and ghost_info["result"]["error"] == "unknown session id", ghost_info
    assert "modes" in opened["result"]
    assert seeded_info["result"]["result"]["permissionPosture"] == "yolo"
    output = {"unknown_mode_accepted": unknown, "before_model_change": before,
              "after_model_change": after, "unknown_session_model_change": ghost,
              "unknown_session_info": ghost_info, "new_session_has_modes": "modes" in opened["result"],
              "yolo_seed_reported_posture": seeded_info["result"]["result"]["permissionPosture"]}
    (AUDIT / "acp-result.json").write_text(json.dumps(output, indent=2))
    print(json.dumps(output, indent=2))
finally:
    process.terminate()
    process.wait(timeout=10)
    stderr.close()
