#!/bin/bash
# RUN THIS YOURSELF in an UNRESTRICTED login terminal (Terminal.app / iTerm2).
# Mira cannot do it for you: four automatic routes were measured and all fail
# (sandboxed bash, open -n -a wrapper, osascript -> Terminal -10004, and
# opening a .command file). See OUTCOME.md round 38.
#
# It only reads process/permission state and writes a redacted JSON receipt;
# it does not launch Studio or call any LLM.
#
#   bash /Users/roylin/code/aster/docs/evidence/studio/2026-11-12-business-function-coverage/specs/recheck-host.sh
#
# Then paste the receipt (or just say "host 恢复了") back to Mira; she reads
# /tmp/aster-visual-e2e/host-recheck.json and resumes the GUI E2E cases.

OUT=/tmp/aster-visual-e2e/host-recheck.json
mkdir -p /tmp/aster-visual-e2e

python3 - <<'PY' > "$OUT" 2>/dev/null
import json
import subprocess

result = {
    "context": "unrestricted login terminal",
    "checks": {},
}

try:
    probe = subprocess.run(
        ["/bin/ps", "-p", "1", "-o", "pid=,comm="],
        capture_output=True, text=True, timeout=10,
    )
    result["checks"]["ps"] = {
        "ok": probe.returncode == 0,
        "stderr": (probe.stderr or "").strip()[:160],
    }
except Exception as error:
    result["checks"]["ps"] = {"ok": False, "stderr": type(error).__name__}

try:
    probe = subprocess.run(
        ["agent-desktop", "list-apps"],
        capture_output=True, text=True, timeout=45,
    )
    payload = json.loads(probe.stdout or "{}")
    error = payload.get("error") or {}
    result["checks"]["list_apps"] = {
        "ok": bool(payload.get("ok")),
        "app_count": len(payload.get("data", {}).get("apps", [])),
        "error_code": error.get("code"),
        "error_kind": (error.get("details") or {}).get("kind"),
        "last_failure": (error.get("details") or {}).get("last_failure"),
    }
except subprocess.TimeoutExpired:
    result["checks"]["list_apps"] = {"ok": False, "error_code": "TIMEOUT"}
except Exception as error:
    result["checks"]["list_apps"] = {"ok": False, "error_code": type(error).__name__}

try:
    probe = subprocess.run(
        ["agent-desktop", "permissions"],
        capture_output=True, text=True, timeout=30,
    )
    payload = json.loads(probe.stdout or "{}")
    states = {
        key: (value or {}).get("state")
        for key, value in (payload.get("data") or {}).items()
        if isinstance(value, dict)
    }
    result["checks"]["permissions"] = {"ok": bool(payload.get("ok")), "states": states}
except Exception as error:
    result["checks"]["permissions"] = {"ok": False, "error_code": type(error).__name__}

print(json.dumps(result, ensure_ascii=False, indent=2))
PY

echo "Receipt written to $OUT"
cat "$OUT"
echo
echo "If checks.ps.ok is true and checks.list_apps.ok is true, tell Mira:"
echo "  'host 恢复了' — she will resume the GUI E2E cases."
echo "If ps.ok is false here too, the block is system-wide and needs a"
echo "restart / TCC reset rather than any test-side change."
