"""Same GPU/binary0-1-0 comparison: basic28, native-tool/long12 and forced-pool13 per profile."""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH=BASE/'xolotl-pool-seed-v112-controller.json'
STATE=dict(unix=time.time(),phase='starting',profiles=[],formal_service_unchanged=True,
           actual1m_input_tested=False,goal_complete=False,performance_300_qualified=False)
with PATH.open('x') as f:f.write(json.dumps(STATE,indent=2)+'\n')


def save():
    STATE['observed_unix']=time.time();p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(PATH)


def run(script,*args):
    p=subprocess.run([sys.executable,'-X','utf8',str(BASE/script),*args],cwd=BASE,capture_output=True,text=True,timeout=100)
    if p.returncode:raise RuntimeError(script+' '+str(args)+' failed: '+p.stderr[-4000:])
    return json.loads(p.stdout)


def wait_model(profile,want,seconds):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        v=run('manage_pool_basic_v108.frozen.py',profile,'observe');q=v.get('service-longctx-v1-result.json',{})
        STATE.update(service_phase=q.get('phase'));save()
        if want=='serving' and q.get('phase')=='serving' and v['exact_supervisor_alive']:return
        if want=='stopped' and q.get('phase')=='stopped' and q.get('api_reaped') and not v['exact_supervisor_alive']:return
        if q.get('phase')=='failed' or want=='serving' and not v['exact_supervisor_alive']:
            raise RuntimeError('exact existing model failed/disappeared; inspect same handle')
        time.sleep(15)
    raise RuntimeError('finite model observation budget exceeded; inspect exact existing process')


def wait_client(script,profile,action,seconds,count,allow_completed_failure=False):
    end=time.monotonic()+seconds;last=None
    while time.monotonic()<end:
        q=run(script,profile,action)
        STATE.update(client_phase=q.get('phase'),client_tests=q.get('tests'));save()
        current=(q.get('phase'),q.get('tests'))
        if current!=last:print(json.dumps(dict(profile=profile,stage=STATE['phase'],phase=current[0],tests=current[1])),flush=True);last=current
        if not q.get('exact_client_alive',False):
            if q.get('tests')!=count:raise RuntimeError('client stopped without all required requests: '+str(q.get('error')))
            if allow_completed_failure:
                if not q.get('completed') or not q.get('cold_reference_qualified'):
                    raise RuntimeError('forced-pool structural/cold reference gate failed: '+str(q.get('error')))
            elif q.get('phase')!='passed':raise RuntimeError('required qualification failed: '+str(q.get('error')))
            return q
        time.sleep(15)
    raise RuntimeError('finite client observation budget exceeded; inspect exact existing handle')


try:
    q=run('observe_natural_long_fixture_v109.frozen.py')
    if q['phase']!='passed' or q['alive']:raise RuntimeError('exact finite CPU tokenizer fixture must pass first')
    for profile in ['pool_off','pool_on','pool_off_repeat']:
        STATE.update(phase='loading_profile',profile=profile);save()
        ip=BASE/('xolotl-pool-service-v108-'+profile+'-intent.json')
        if profile=='pool_off':
            r=json.loads(ip.read_text())
            if r['phase']!='launched' or not r.get('process'):raise RuntimeError('exact pre-existing first profile handle required')
        else:run('launch_pool_service_v108.frozen.py',profile)
        wait_model(profile,'serving',420)
        STATE['phase']='basic28';save()
        if not (BASE/('xolotl-pool-service-v108-'+profile+'-qualification-intent.json')).exists():
            run('manage_pool_basic_v108.frozen.py',profile,'launch_client')
        basic=wait_client('manage_pool_basic_v108.frozen.py',profile,'observe_client',1150,28)
        STATE['phase']='native_tool_long12';save()
        if not (BASE/('xolotl-pool-extended-v108-'+profile+'-intent.json')).exists():
            run('manage_pool_extended_v108.frozen.py',profile,'launch')
        extended=wait_client('manage_pool_extended_v108.frozen.py',profile,'observe',1350,12)
        if not extended.get('native_tool_roundtrip_passed') or not extended.get('reference_matched'):
            raise RuntimeError('native tool and matched original1024-output references must pass')
        STATE['phase']='forced_pool_long13';save()
        run('manage_pool_long_v111.frozen.py',profile,'launch')
        pool=wait_client('manage_pool_long_v111.frozen.py',profile,'observe',950,13,True)
        STATE['profiles'].append(dict(profile=profile,basic28_passed=True,native_tool_long12_passed=True,
            pool13_phase=pool['phase'],pool13_completed=True,cases=pool['cases'],short_summary=basic.get('short_summary'),long_summary=extended.get('long_summary')))
        save()
        STATE['phase']='draining_completed_profile';save();run('manage_pool_basic_v108.frozen.py',profile,'drain')
        wait_model(profile,'stopped',120)
    STATE.update(phase='comparison_completed',finished_unix=time.time());save()
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
