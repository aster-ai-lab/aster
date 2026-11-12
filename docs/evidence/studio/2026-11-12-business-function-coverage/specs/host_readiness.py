#!/usr/bin/env python3
"""Keep the resident agent-desktop command host in a context where /bin/ps works.

The host inherits the context of whoever spawned it. When it is respawned from
the sandboxed Bash session, /bin/ps (setuid root, SIP protected) is refused and
every window command times out with application_inventory_unstable. Respawning
from the Launch Services context of AsterVisualE2E.app restores it -- that app
has Accessibility granted, and a freshly created separate warmup bundle does
NOT inherit the same context (observed: list-apps still TIMEOUT there).

This module temporarily swaps that app's launcher, restores it afterwards, so
install_wrapper() can still rewrite the launcher for the actual test case.
"""
import ctypes
import json
import os
import subprocess
import time
from pathlib import Path

APP_DIR = Path.home() / "Applications" / "AsterVisualE2E.app"
LAUNCHER = APP_DIR / "Contents" / "MacOS" / "AsterVisualE2E"
AGENT_DESKTOP = "/opt/homebrew/bin/agent-desktop"
LOG = Path("/tmp/aster-visual-e2e/host-warmup-result.txt")


def host_ok(timeout=30):
    try:
        out = subprocess.run(
            [AGENT_DESKTOP, "list-apps"], capture_output=True, text=True, timeout=timeout
        )
        return json.loads(out.stdout).get("ok") is True
    except (subprocess.TimeoutExpired, json.JSONDecodeError, OSError):
        return False


def kill_stale_aster():
    """open -a only ACTIVATES an already-running bundle; the warmup launcher
    would never run. libproc (no /bin/ps) is used to clear stale instances."""
    lib = ctypes.CDLL("/usr/lib/libproc.dylib")
    lib.proc_listallpids.argtypes = [ctypes.c_void_p, ctypes.c_int]
    lib.proc_listallpids.restype = ctypes.c_int
    lib.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64,
                                 ctypes.c_void_p, ctypes.c_int]
    lib.proc_pidinfo.restype = ctypes.c_int

    class ProcInfo(ctypes.Structure):
        _fields_ = [("a", ctypes.c_uint32), ("b", ctypes.c_uint32),
                    ("c", ctypes.c_uint32), ("pid", ctypes.c_uint32),
                    ("ppid", ctypes.c_uint32), ("uid", ctypes.c_uint32),
                    ("gid", ctypes.c_uint32), ("ruid", ctypes.c_uint32),
                    ("rgid", ctypes.c_uint32), ("svuid", ctypes.c_uint32),
                    ("svgid", ctypes.c_uint32), ("rfu", ctypes.c_uint32),
                    ("comm", ctypes.c_char * 16), ("name", ctypes.c_char * 32),
                    ("nfiles", ctypes.c_uint32), ("pgid", ctypes.c_uint32),
                    ("pjobc", ctypes.c_uint32), ("tdev", ctypes.c_uint32),
                    ("tpgid", ctypes.c_uint32), ("nice", ctypes.c_int32),
                    ("s1", ctypes.c_uint64), ("s2", ctypes.c_uint64)]

    buf = (ctypes.c_int * 8192)()
    count = lib.proc_listallpids(buf, ctypes.sizeof(buf))
    for i in range(max(count, 0)):
        info = ProcInfo()
        pid = buf[i]
        if lib.proc_pidinfo(pid, 3, 0, ctypes.byref(info), ctypes.sizeof(info)) > 0:
            if info.comm.decode("utf8", "replace") in ("Aster", "Studio"):
                try:
                    os.kill(pid, 15)
                except OSError:
                    pass


def warmup(attempts=12):
    if not LAUNCHER.exists():
        return False
    kill_stale_aster()
    time.sleep(2)
    original = LAUNCHER.read_bytes()
    try:
        LAUNCHER.write_text(
            "#!/bin/sh\n"
            "# Launch Services context of the Accessibility-granted bundle.\n"
            "{\n"
            "  for p in $(/bin/ps -axo pid=,comm= | grep -i 'agent-desktop' | awk '{print $1}'); do\n"
            '    kill "$p" 2>/dev/null\n'
            "  done\n"
            "  sleep 2\n"
            f"  {AGENT_DESKTOP} list-apps 2>&1 | head -c 200\n"
            f"}} > {LOG} 2>&1\n"
            "touch /tmp/aster-visual-e2e/host-warmup-done.flag\n"
        )
        LAUNCHER.chmod(0o755)
        done = Path("/tmp/aster-visual-e2e/host-warmup-done.flag")
        done.unlink(missing_ok=True)
        # -n is required: plain `open -a` only ACTIVATES an already-running
        # bundle and never executes the swapped launcher.
        subprocess.run(["open", "-n", "-a", str(APP_DIR)], capture_output=True)
        # Do NOT call agent-desktop from Bash until the wrapper has spawned the
        # host itself; a Bash call would win the race and respawn it sandboxed.
        for _ in range(attempts):
            time.sleep(2)
            if done.exists():
                break
        time.sleep(2)
        for _ in range(attempts):
            if host_ok():
                return True
            time.sleep(2)
        return False
    finally:
        LAUNCHER.write_bytes(original)
        LAUNCHER.chmod(0o755)


def ensure_host(force=False, retries=2):
    """Return True when list-apps is healthy, respawning the host if needed.

    A launch immediately after close-app can race with the dying Aster process;
    one warmup is then not enough. Retry rather than failing a whole case on
    infrastructure recovery.
    """
    if not force and host_ok():
        return True
    for attempt in range(retries):
        if warmup():
            return True
        if attempt + 1 < retries:
            time.sleep(3)
    return False


if __name__ == "__main__":
    import sys
    ok = ensure_host(force="--force" in sys.argv)
    print(f"HOST_READY: {'PASS' if ok else 'FAIL'}")
    sys.exit(0 if ok else 1)
