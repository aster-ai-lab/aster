"""Bounded sequential A/B controller. Existing handles are observed, never restarted."""
import json
import subprocess
import sys
import time
from pathlib import Path

B=Path(__file__).resolve().parent
state=dict(unix=time.time(),phase='starting',profiles=[],formal_service_unchanged=True)
deadline=time.monotonic()+3600

def save():
    (B/'xolotl-resident-compare-v37-controller.json').write_text(json.dumps(state,indent=2)+'\n')

def command(script,*args):
    p=subprocess.run([sys.executable,'-X','utf8',str(B/script),*args],capture_output=True,text=True,timeout=110)
    if p.returncode:
        raise RuntimeError(script+' failed: '+p.stderr[-3000:])
    return json.loads(p.stdout)

def pause():
    if time.monotonic()>deadline:raise RuntimeError('finite comparison controller budget exceeded; inspect existing exact handles')
    time.sleep(10)

try:
    for profile in ['v15_control','v35_keep','v35_packed']:
        state.update(phase='starting_profile',current_profile=profile);save()
        if not (B/('xolotl-resident-compare-v37-'+profile+'-intent.json')).exists():
            command('launch_xolotl_resident_compare_service_v37.frozen.py',profile)
        while True:
            v=command('manage_xolotl_resident_compare_v37.frozen.py',profile,'observe')
            q=v.get('service-longctx-v1-result.json',{})
            state['service_phase']=q.get('phase');save()
            if q.get('phase')=='serving':break
            if q.get('phase') in ('failed','stopped') or not v['exact_supervisor_alive']:
                raise RuntimeError('exact comparison service terminal before qualification')
            pause()
        if not (B/('xolotl-resident-compare-v37-'+profile+'-qualification-intent.json')).exists():
            command('manage_xolotl_resident_compare_v37.frozen.py',profile,'launch_client')
        last=None
        while True:
            v=command('manage_xolotl_resident_compare_v37.frozen.py',profile,'observe_client')
            state.update(phase='qualifying',client_phase=v['phase'],client_tests=v['tests']);save()
            current=(v['phase'],v['tests'])
            if current!=last:
                print(json.dumps({'profile':profile,'phase':v['phase'],'tests':v['tests'],'client_alive':v['exact_client_alive']}),flush=True);last=current
            if v['phase']=='passed' and not v['exact_client_alive']:break
            if v['phase']=='failed' or (not v['exact_client_alive'] and v['phase']!='passed'):
                raise RuntimeError('exact public client terminal without completed qualification: '+str(v.get('error')))
            pause()
        state['profiles'].append(v);save()
        print(json.dumps({'profile':profile,'phase':'passed','short_summary':v['short_summary'],'long_tests':v['long_tests']}),flush=True)
        if profile!='v35_packed':
            if not (B/('xolotl-resident-compare-v37-'+profile+'-drain.json')).exists():
                command('manage_xolotl_resident_compare_v37.frozen.py',profile,'drain')
            while True:
                v=command('manage_xolotl_resident_compare_v37.frozen.py',profile,'observe')
                q=v.get('service-longctx-v1-result.json',{})
                if q.get('phase')=='stopped' and q.get('api_reaped') and not v['exact_supervisor_alive']:break
                if q.get('phase')=='failed':raise RuntimeError('comparison service failed while draining')
                pause()
    state.update(phase='passed',finished_unix=time.time(),last_packed_service_kept_for_fullwindow_test=True);save()
    print(json.dumps({'phase':'passed','profiles':len(state['profiles']),'full1m_candidate_tested':False}),flush=True)
except BaseException as e:
    state.update(phase='needs_inspection',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
