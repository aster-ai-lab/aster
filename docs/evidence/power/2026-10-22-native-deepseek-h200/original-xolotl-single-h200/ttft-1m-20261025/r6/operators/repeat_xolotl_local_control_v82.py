"""After exact decoder clients finish, repeat the original local-table seven-request control."""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH=BASE/'xolotl-engram-local-repeat-v82-controller.json'
STATE=dict(unix=time.time(),phase='waiting_exact_v78',formal_service_unchanged=True,actual1m_input_tested=False)
with PATH.open('x') as output:output.write(json.dumps(STATE,indent=2)+'\n')


def save():
    STATE['observed_unix']=time.time();tmp=PATH.with_suffix('.tmp');tmp.write_text(json.dumps(STATE,indent=2)+'\n');tmp.replace(PATH)


def run(script,*args):
    value=subprocess.run([sys.executable,'-X','utf8',str(BASE/script),*args],cwd=BASE,capture_output=True,text=True,timeout=100)
    if value.returncode:raise RuntimeError(script+' exit'+str(value.returncode)+': '+value.stderr[-3000:])


def wait_model(lut,target,seconds):
    script='manage_xolotl_engram_lut_service_v75.frozen.py' if lut else 'manage_xolotl_engram_local_repeat_service_v79.frozen.py'
    profile='lut_local' if lut else 'local_repeat'
    path=BASE/('xolotl-engram-lut-v75-lut_local-latest.json' if lut else 'xolotl-engram-local-repeat-v79-local_repeat-latest.json')
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        run(script,profile,'observe');v=json.loads(path.read_text());q=v.get('service-longctx-v1-result.json',{});STATE['service_phase']=q.get('phase');save()
        if target=='stopped' and q.get('phase')=='stopped' and q.get('api_reaped') and not v['exact_supervisor_alive']:return
        if target=='serving' and q.get('phase')=='serving' and v['exact_supervisor_alive']:return
        if q.get('phase')=='failed' or target=='serving' and not v['exact_supervisor_alive']:raise RuntimeError('exact model failed/disappeared; inspect it')
        time.sleep(10)
    raise RuntimeError('model observation budget exceeded; inspect same handle')


try:
    end=time.monotonic()+360
    while time.monotonic()<end:
        q=json.loads((BASE/'xolotl-engram-lut-v78-controller.json').read_text())
        if q['phase']=='requires_inspection':raise RuntimeError('exact prior decoder controller requires inspection')
        if q['phase']=='passed':break
        time.sleep(10)
    else:raise RuntimeError('exact V78 did not finish within the observation budget; inspect its existing handle')
    for script,path in [('manage_xolotl_engram_lut_natural_v76.frozen.py','xolotl-engram-lut-natural-v76-lut_local-latest.json'),('manage_xolotl_engram_lut_extended_v77.frozen.py','xolotl-engram-lut-extended-v77-lut_local-latest.json')]:
        run(script,'lut_local','observe');v=json.loads((BASE/path).read_text())
        if v['exact_client_alive'] or v['result']['phase']!='passed':raise RuntimeError('exact decoder clients must pass and be gone before planned drain')
    run('collect_xolotl_engram_numeric_v71.py')
    STATE['phase']='draining_completed_decoder';save();run('manage_xolotl_engram_lut_service_v75.frozen.py','lut_local','drain');wait_model(True,'stopped',100)
    STATE['phase']='launching_original_local_repeat';save();run('launch_xolotl_engram_local_repeat_v79.frozen.py','local_repeat');wait_model(False,'serving',180)
    STATE['phase']='natural_repeat';save();run('manage_xolotl_engram_local_repeat_v80.frozen.py','local_repeat','launch')
    end=time.monotonic()+620
    while time.monotonic()<end:
        run('manage_xolotl_engram_local_repeat_v80.frozen.py','local_repeat','observe')
        v=json.loads((BASE/'xolotl-engram-local-repeat-v80-local_repeat-latest.json').read_text());q=v.get('result',{});STATE.update(client_phase=q.get('phase'),client_tests=len(q.get('tests',[])));save()
        if q.get('phase')=='failed':raise RuntimeError('exact original local repeat failed: '+str(q.get('error')))
        if not v['exact_client_alive']:
            if q.get('phase')!='passed' or len(q['tests'])!=7:raise RuntimeError('exact original local repeat stopped without all7 gates')
            STATE.update(phase='passed',cases=q['cases'],finished_unix=time.time());save();break
        time.sleep(15)
    else:raise RuntimeError('exact repeat observation budget expired; inspect same handle')
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
