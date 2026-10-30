"""Resume the prepared width screen after a planned local-only interruption; remote V45 is unchanged."""
import json
import subprocess
import sys
import time
from pathlib import Path

B=Path(__file__).resolve().parent
state=dict(unix=time.time(),phase='starting',profiles=[],formal_service_unchanged=True)
deadline=time.monotonic()+7000

def save():
    (B/'xolotl-dense-subbatch-v47r1-controller.json').write_text(json.dumps(state,indent=2)+'\n')

def command(script,*args):
    p=subprocess.run([sys.executable,'-X','utf8',str(B/script),*args],capture_output=True,text=True,timeout=110)
    if p.returncode:
        raise RuntimeError(script+' failed: '+p.stderr[-3000:])
    return json.loads(p.stdout)

def pause():
    if time.monotonic()>deadline:raise RuntimeError('finite comparison controller budget exceeded; inspect existing exact handles')
    time.sleep(10)

try:
    state.update(phase='waiting_for_exact_v45');save()
    last=None
    while True:
        v=command('manage_xolotl_indexer_fullwindow_suite_v45.frozen.py','observe')
        current=(v['phase'],v.get('current_stage'))
        state.update(phase='waiting_for_exact_v45',v45_phase=v['phase'],v45_stage=v.get('current_stage'));save()
        if current!=last:
            print(json.dumps({'waiting_for':'V45','phase':v['phase'],'stage':v.get('current_stage'),'exact_client_alive':v['exact_client_alive']}),flush=True);last=current
        if v['phase']=='passed' and not v['exact_client_alive']:break
        if v['phase']=='failed' or not v['exact_client_alive']:raise RuntimeError('exact V45 suite terminal without passing; no sub-batch mutation')
        pause()
    if not (B/'xolotl-indexer-v45-drain.json').exists():
        command('drain_xolotl_indexer_after_fullwindow_v45.frozen.py')
    while True:
        v=command('manage_xolotl_indexer_service_v44.frozen.py','observe')
        q=v.get('service-longctx-v1-result.json',{})
        if q.get('phase')=='stopped' and q.get('api_reaped') and not v['exact_supervisor_alive']:break
        if q.get('phase')=='failed':raise RuntimeError('V44 failed while draining; inspect exact handles')
        pause()
    for profile in ['dense256','dense512','dense128_repeat']:
        state.update(phase='starting_profile',current_profile=profile);save()
        if not (B/('xolotl-dense-subbatch-v47-'+profile+'-intent.json')).exists():
            command('launch_xolotl_dense_subbatch_v47.frozen.py',profile)
        while True:
            v=command('manage_xolotl_dense_subbatch_v47.frozen.py',profile,'observe')
            q=v.get('service-longctx-v1-result.json',{})
            state['service_phase']=q.get('phase');save()
            if q.get('phase')=='serving':break
            if q.get('phase') in ('failed','stopped') or not v['exact_supervisor_alive']:
                raise RuntimeError('exact comparison service terminal before qualification')
            pause()
        if not (B/('xolotl-dense-subbatch-v47-'+profile+'-qualification-intent.json')).exists():
            command('manage_xolotl_dense_subbatch_v47.frozen.py',profile,'launch_client')
        last=None
        while True:
            v=command('manage_xolotl_dense_subbatch_v47.frozen.py',profile,'observe_client')
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
        if profile!='dense128_repeat':
            if not (B/('xolotl-dense-subbatch-v47-'+profile+'-drain.json')).exists():
                command('manage_xolotl_dense_subbatch_v47.frozen.py',profile,'drain')
            while True:
                v=command('manage_xolotl_dense_subbatch_v47.frozen.py',profile,'observe')
                q=v.get('service-longctx-v1-result.json',{})
                if q.get('phase')=='stopped' and q.get('api_reaped') and not v['exact_supervisor_alive']:break
                if q.get('phase')=='failed':raise RuntimeError('comparison service failed while draining')
                pause()
    state.update(phase='passed',finished_unix=time.time(),last_control128_service_kept_for_next_work=True);save()
    print(json.dumps({'phase':'passed','profiles':len(state['profiles']),'full1m_candidate_tested':False}),flush=True)
except BaseException as e:
    state.update(phase='needs_inspection',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
