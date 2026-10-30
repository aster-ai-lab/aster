"""Observe the exact recovered repeat model and execute its seven unchanged correctness/timing requests once."""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH=BASE/'xolotl-engram-local-repeat-v84-controller.json'
STATE=dict(unix=time.time(),phase='loading_exact_recovered_repeat',formal_service_unchanged=True,actual1m_input_tested=False,original_v82_refusal_retained=True)
with PATH.open('x') as output:output.write(json.dumps(STATE,indent=2)+'\n')


def save():
    STATE['observed_unix']=time.time();tmp=PATH.with_suffix('.tmp');tmp.write_text(json.dumps(STATE,indent=2)+'\n');tmp.replace(PATH)


def run(script,*args):
    p=subprocess.run([sys.executable,'-X','utf8',str(BASE/script),*args],cwd=BASE,capture_output=True,text=True,timeout=100)
    if p.returncode:raise RuntimeError(script+' exit'+str(p.returncode)+': '+p.stderr[-3000:])


try:
    end=time.monotonic()+180
    while time.monotonic()<end:
        run('manage_xolotl_engram_local_repeat_service_v79.frozen.py','local_repeat','observe')
        v=json.loads((BASE/'xolotl-engram-local-repeat-v79-local_repeat-latest.json').read_text());q=v.get('service-longctx-v1-result.json',{})
        STATE['service_phase']=q.get('phase');save()
        if q.get('phase')=='serving' and v['exact_supervisor_alive']:break
        if q.get('phase')=='failed' or not v['exact_supervisor_alive']:raise RuntimeError('exact recovered model failed/disappeared')
        time.sleep(10)
    else:raise RuntimeError('exact recovered model observation budget exceeded; inspect it')
    STATE['phase']='natural_repeat';save();run('manage_xolotl_engram_local_repeat_v80.frozen.py','local_repeat','launch')
    end=time.monotonic()+440
    while time.monotonic()<end:
        run('manage_xolotl_engram_local_repeat_v80.frozen.py','local_repeat','observe')
        v=json.loads((BASE/'xolotl-engram-local-repeat-v80-local_repeat-latest.json').read_text());q=v.get('result',{})
        STATE.update(client_phase=q.get('phase'),client_tests=len(q.get('tests',[])));save()
        if q.get('phase')=='failed':raise RuntimeError('exact seven-request control failed: '+str(q.get('error')))
        if not v['exact_client_alive']:
            if q.get('phase')!='passed' or len(q['tests'])!=7:raise RuntimeError('exact control stopped without all7 gates')
            STATE.update(phase='passed',cases=q['cases'],finished_unix=time.time());save();break
        time.sleep(15)
    else:raise RuntimeError('exact control observation budget exceeded; inspect same handle')
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
