"""Wait for unchanged C11 gates, switch VIP once, then verify Windows and start observation."""
import json
from pathlib import Path
import subprocess
import sys
import time
import urllib.request

B=Path(__file__).resolve().parent
PATH=B/'xolotl-stable-recovery-v158-controller.json'
if PATH.exists():raise RuntimeError('exact recovery follow-up has a handle; inspect it')
STATE=dict(unix=time.time(),phase='waiting_qualification',optimization_restarted=False,maximum_h200_roles=2,goal_complete=False)
def save():
    p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(PATH)
def command(script):
    result=subprocess.run([sys.executable,'-X','utf8',str(B/script)],cwd=B,capture_output=True,text=True,timeout=200)
    if result.returncode:raise RuntimeError(script+' failed: '+result.stderr[-4000:])
    return json.loads(result.stdout)
try:
    save();deadline=time.monotonic()+3600;last=None
    while time.monotonic()<deadline:
        q=json.loads((B/'xolotl-stable-recovery-v157-controller.json').read_text())
        stage=(q['phase'],q.get('client_tests'),q.get('edge_phase'))
        STATE.update(qualification_phase=q['phase'],qualification_tests=q.get('client_tests'),edge_phase=q.get('edge_phase'),observed_unix=time.time());save()
        if stage!=last:print(json.dumps(STATE),flush=True);last=stage
        if q['phase']=='passed':break
        if q['phase']=='needs_inspection':raise RuntimeError('exact C11 qualification requires inspection: '+str(q.get('error')))
        time.sleep(15)
    else:raise RuntimeError('finite qualification observer elapsed; keep exact handles')
    STATE['phase']='promoting';save()
    promotion=B/'xolotl-stable-recovery-v157-promotion.json'
    if not promotion.exists():
        STATE['switch']=command('promote_xolotl_stable_recovery_v157.frozen.py');save()
    else:
        q=json.loads(promotion.read_text())
        if q['phase'] not in ['switched','windows_validation_pending','promoted']:raise RuntimeError('existing VIP switch outcome uncertain; never replay mutation')
    STATE['phase']='waiting_readonly_windows_health';save()
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}));deadline=time.monotonic()+90
    while time.monotonic()<deadline:
        try:
            with opener.open('http://10.102.93.68/health',timeout=5) as response:
                body=json.loads(response.read(65536))
            if response.status==200 and body.get('loaded') and body.get('max_context')==1048576:break
        except OSError:pass
        time.sleep(5)
    else:raise RuntimeError('already-switched VIP health not propagated; inspect without another switch')
    if json.loads(promotion.read_text())['phase']!='promoted':
        STATE['windows']=command('finalize_xolotl_stable_recovery_v157.frozen.py');save()
    STATE['phase']='readiness_watch';save()
    if not (B/'xolotl-readiness-watch-v159-intent.json').exists():
        STATE['watch']=command('launch_xolotl_readiness_watch_v159.frozen.py');save()
    STATE.update(phase='promoted',finished_unix=time.time());save()
    print(json.dumps(dict(phase='promoted',base_url='http://10.102.93.68/v1',same_existing_key=True,optimization_restarted=False)),flush=True)
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error)[:4000],finished_unix=time.time());save();raise
