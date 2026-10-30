"""Same GPU/binary0-1-0 comparison: basic28, native-tool/long12 and forced-pool13 per profile."""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH=BASE/'xolotl-pool-seed-v112r2-controller.json'
STATE=dict(unix=time.time(),phase='starting',profiles=[],formal_service_unchanged=True,
           actual1m_input_tested=False,goal_complete=False,performance_300_qualified=False,
           resumes='xolotl-pool-seed-v112-controller.json',
           resume_reason='Retained local Windows atomic-replace refusal; no GPU/client failure or restart')
STATE.update(profiles=[{'profile': 'pool_off', 'basic28_passed': True, 'native_tool_long12_passed': True, 'pool13_phase': 'failed', 'pool13_completed': True, 'cases': [{'case': 'technical-long8244', 'actual_input_tokens': 8244, 'cold_seconds': 37.10260251071304, 'cold_ttft_seconds': 17.466755617875606, 'restored_seconds': 25.522302580066025, 'restored_ttft_seconds': 0.018420793116092682, 'repeated_seconds': 22.976548321079463, 'pool_restore_logged': True, 'pool_seed_restore_logged': False, 'restored_matches_control_cold': False, 'repeated_matches_control_cold': False}, {'case': 'technical-long25671', 'actual_input_tokens': 25671, 'cold_seconds': 52.30066881002858, 'cold_ttft_seconds': 29.44372253632173, 'restored_seconds': 26.073732452001423, 'restored_ttft_seconds': 0.048779203090816736, 'repeated_seconds': 23.168938744347543, 'pool_restore_logged': True, 'pool_seed_restore_logged': False, 'restored_matches_control_cold': False, 'repeated_matches_control_cold': False}], 'short_summary': {'measured_requests': 16, 'ttft_p50_seconds': 0.1250994282308966, 'ttft_p95_seconds': 0.15754788427148014, 'api_wall_completion_tok_s': 85.24627193967657, 'stream_interval_tok_s_estimate': 94.86655441747784}, 'long_summary': {'requests': 8, 'completion_tokens': 8192, 'wall_seconds': 107.95972601091489, 'api_wall_completion_tok_s': 75.88014811348981, 'stream_interval_tok_s_estimate': 75.90878406361936, 'ttft_p50_seconds': 0.010812310501933098, 'ttft_min_seconds': 0.01010194281116128, 'ttft_max_seconds': 0.011443051043897867}}, {'profile': 'pool_on', 'basic28_passed': True, 'native_tool_long12_passed': True, 'pool13_phase': 'failed', 'pool13_completed': True, 'cases': [{'case': 'technical-long8244', 'actual_input_tokens': 8244, 'cold_seconds': 33.62289581075311, 'cold_ttft_seconds': 14.940016196109354, 'restored_seconds': 17.743096912279725, 'restored_ttft_seconds': 0.028363704215735197, 'repeated_seconds': 14.855859884992242, 'pool_restore_logged': True, 'pool_seed_restore_logged': True, 'restored_matches_control_cold': False, 'repeated_matches_control_cold': False}, {'case': 'technical-long25671', 'actual_input_tokens': 25671, 'cold_seconds': 46.123465979937464, 'cold_ttft_seconds': 27.07826703088358, 'restored_seconds': 17.348084321711212, 'restored_ttft_seconds': 0.05694234697148204, 'repeated_seconds': 15.2323676222004, 'pool_restore_logged': True, 'pool_seed_restore_logged': True, 'restored_matches_control_cold': False, 'repeated_matches_control_cold': False}], 'short_summary': {'measured_requests': 16, 'ttft_p50_seconds': 0.1331204674206674, 'ttft_p95_seconds': 0.16394843976013362, 'api_wall_completion_tok_s': 81.59944632673421, 'stream_interval_tok_s_estimate': 93.2901974790026}, 'long_summary': {'requests': 8, 'completion_tokens': 8192, 'wall_seconds': 108.71292388765141, 'api_wall_completion_tok_s': 75.35442619928025, 'stream_interval_tok_s_estimate': 75.38407085733421, 'ttft_p50_seconds': 0.010500052012503147, 'ttft_min_seconds': 0.010207006242126226, 'ttft_max_seconds': 0.01165191875770688}}],resumes='xolotl-pool-seed-v112r1-controller.json',
             resume_reason='Exact last profile recovered only after absent root/fresh resource proof; completed measurements preserved')
with PATH.open('x') as f:f.write(json.dumps(STATE,indent=2)+'\n')


def save():
    STATE['observed_unix']=time.time();p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\n')
    for attempt in range(12):
        try:
            p.replace(PATH);return
        except PermissionError:
            if attempt==11:raise
            time.sleep(0.1*(attempt+1))


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
    for profile in ['pool_off_repeat']:
        STATE.update(phase='loading_profile',profile=profile);save()
        ip=BASE/('xolotl-pool-service-v108-'+profile+'-intent.json')
        if profile=='pool_off_repeat':
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
        if not (BASE/('xolotl-pool-long-v111-'+profile+'-intent.json')).exists():
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
