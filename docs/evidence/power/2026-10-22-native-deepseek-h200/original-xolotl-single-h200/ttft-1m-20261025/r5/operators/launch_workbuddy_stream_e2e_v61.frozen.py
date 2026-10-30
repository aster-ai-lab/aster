"""Launch a finite installed-client conversation test, no listening ports or system setting changes."""
import ctypes,hashlib,json,subprocess,sys,time
from ctypes import wintypes
from pathlib import Path
BASE=Path(__file__).resolve().parent;ip=BASE/'workbuddy-stream-e2e-v61-intent.json'
if ip.exists():raise RuntimeError('WorkBuddy stream E2E intent exists; inspect it')
promotion=json.loads((BASE/'xolotl-stable-recovery-v58-promotion.json').read_text())
if promotion['phase']!='promoted' or not promotion['backend_verified_by_unique_nonce_and_kv_reuse']:raise RuntimeError('actual Windows VIP recovery must pass first')
p=json.loads((BASE/'workbuddy-builtin-e2e-v1-plan.json').read_text());private=Path(p['private_output']).parent/'workbuddy-stream-e2e-v61';private.mkdir();p['private_output']=str(private);(BASE/'workbuddy-stream-e2e-v61-plan.json').write_text(json.dumps(p,indent=2)+'\n')
worker=BASE/'workbuddy_stream_e2e_worker_v61.frozen.py';r=dict(unix=time.time(),phase='launch_requested',worker_sha256=hashlib.sha256(worker.read_bytes()).hexdigest(),same_process_two_turns=True,no_session_persistence=True,gui_test=False);ip.write_text(json.dumps(r,indent=2)+'\n')
with (private/'worker.stdout.log').open('xb') as a,(private/'worker.stderr.log').open('xb') as b:q=subprocess.Popen([sys.executable,'-X','utf8',str(worker)],cwd=BASE,stdin=subprocess.DEVNULL,stdout=a,stderr=b,creationflags=subprocess.CREATE_NO_WINDOW)
ft=[wintypes.FILETIME() for _ in range(4)];fn=ctypes.windll.kernel32.GetProcessTimes;fn.argtypes=[wintypes.HANDLE]+[ctypes.POINTER(wintypes.FILETIME)]*4
if not fn(wintypes.HANDLE(int(q._handle)),*[ctypes.byref(x) for x in ft]):raise ctypes.WinError()
r.update(phase='launched',pid=q.pid,created_filetime=(ft[0].dwHighDateTime<<32)|ft[0].dwLowDateTime);ip.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
