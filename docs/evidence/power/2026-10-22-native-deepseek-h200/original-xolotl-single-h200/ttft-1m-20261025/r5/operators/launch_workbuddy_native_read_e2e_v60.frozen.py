"""Run once after actual Windows VIP recovery; no config edits, forwarding or session reuse."""
import ctypes
import hashlib
import json
import subprocess
import sys
import time
from ctypes import wintypes
from pathlib import Path

BASE = Path(__file__).resolve().parent
intent = BASE / 'workbuddy-native-read-e2e-v60-intent.json'
if intent.exists():
    raise RuntimeError('actual WorkBuddy Read test has an exact handle; observe it')
promotion = json.loads((BASE / 'xolotl-stable-recovery-v58-promotion.json').read_text())
if promotion['phase'] != 'promoted' or not promotion['backend_verified_by_unique_nonce_and_kv_reuse']:
    raise RuntimeError('same owned VIP must pass direct Windows target-cache generation first')
old = json.loads((BASE / 'workbuddy-builtin-e2e-v1-plan.json').read_text())
private = Path(old['private_output']).parent / 'workbuddy-native-read-e2e-v60'
private.mkdir()
cwd = BASE / 'workbuddy-public-read-probe-v60'
cwd.mkdir()
fixture = cwd / 'public-arithmetic.txt'
with fixture.open('x') as output:
    output.write('Public H200 WorkBuddy native Read probe.\nleft=173\nright=269\n')
plan = dict(node=old['node'], entry=old['entry'], model=old['model'], base_url=old['base_url'],
            cwd=str(cwd), public_fixture=str(fixture), private_output=str(private))
(BASE / 'workbuddy-native-read-e2e-v60-plan.json').write_text(json.dumps(plan, indent=2) + '\n')
worker = BASE / 'workbuddy_native_read_e2e_worker_v60.frozen.py'
record = dict(unix=time.time(), phase='launch_requested',
              worker_sha256=hashlib.sha256(worker.read_bytes()).hexdigest(),
              public_fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),
              no_session_persistence=True, gui_test=False, tool_profile='Read only')
with intent.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
with (private / 'worker.stdout.log').open('xb') as output, (private / 'worker.stderr.log').open('xb') as error:
    process = subprocess.Popen([sys.executable, '-X', 'utf8', str(worker)], cwd=BASE,
        stdin=subprocess.DEVNULL, stdout=output, stderr=error, creationflags=subprocess.CREATE_NO_WINDOW)
times = [wintypes.FILETIME() for _ in range(4)]
function = ctypes.windll.kernel32.GetProcessTimes
function.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
if not function(wintypes.HANDLE(int(process._handle)), *[ctypes.byref(value) for value in times]):
    raise ctypes.WinError()
record.update(phase='launched', pid=process.pid,
              created_filetime=(times[0].dwHighDateTime << 32) | times[0].dwLowDateTime)
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
