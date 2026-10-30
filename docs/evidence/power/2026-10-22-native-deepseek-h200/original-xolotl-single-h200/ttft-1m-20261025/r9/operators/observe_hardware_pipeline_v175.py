"""Fresh formal and optimization resource identity plus bounded existing runtime progress."""
import json
import time
import urllib.request
from xolotl_trial_common import BASE,observe as observe_optimization
from xolotl_serving_common import observe as observe_formal,jump

def read(name):return json.loads((BASE/name).read_text())
checkpoint=read('xolotl-serving-and-optimization-next-iteration.json')
formal=read(checkpoint['active_service']['intent'])
watch=read(checkpoint['active_service']['readiness_observer'])
optimization=read('xolotl-single1m-cont10-lease-pending.json')
f=observe_formal();o=observe_optimization()
if f['job_uid']!=formal['allocation_uid'] or o['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or f['job_uid']==o['job_uid']:
    raise RuntimeError('exact independent formal/optimization allocation identity differs')
pipeline_name='xolotl-fullwindow-hardware-v173r1-controller.json' if (BASE/'xolotl-fullwindow-hardware-v173r1-controller.json').exists() else 'xolotl-fullwindow-hardware-v173-controller.json'
pipeline=read(pipeline_name)
profiler=BASE/'xolotl-ncu-actual1m-v172r1-intent.json'
if not profiler.exists():profiler=BASE/'xolotl-ncu-actual1m-v172-intent.json'
runtime=read(profiler.name) if profiler.exists() else read('xolotl-literal-service-v149-literal-intent.json')
kind='hardware_profile' if profiler.exists() else 'literal_API'
code='''import json,time
from pathlib import Path
watch=@WATCH@;runtime=@RUNTIME@;kind=@KIND@
result=dict(unix=time.time(),inference_sent=False,gpu_policy_changed=False)
root=Path(watch['root']);p=root/'watch-result.json'
if root.resolve()!=root or root.stat().st_uid!=10250 or p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded formal watch identity differs')
w=json.loads(p.read_text())
if w['allocation_uid']!=@FORMAL_UID@:raise RuntimeError('fresh formal watch allocation differs')
result['formal_watch']=w;result['formal_watch_age_seconds']=time.time()-w['unix']
root=Path(runtime['root'])
if root.exists():
 if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact optimization runtime owner differs')
 name='engine.stderr.log' if kind=='hardware_profile' else 'engine.log';p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>8*1024**2:raise RuntimeError('bounded optimization engine log exceeded')
  lines=p.read_text(errors='replace').splitlines()
  result['prefill_progress']=[line for line in lines if line.startswith('aster prefill phase: phase=progress total=1047544 ')][-3:]
  result['prompt_phases']=[line for line in lines if line.startswith('serve: prompt phases:')][-3:]
  result['decode_phases']=[line for line in lines if line.startswith(('serve: DSpark','serve: generation','serve: pass split:','aster serve profile:'))][-6:]
 if kind=='hardware_profile':
  p=root/'result.json'
  if p.exists():
   if p.is_symlink() or p.stat().st_size>2*1024**2:raise RuntimeError('bounded profiler result exceeded')
   q=json.loads(p.read_text());result['profiler']={key:q.get(key) for key in ['phase','current_request','current_role','current_state','hardware_counters_collected','error','finished_unix']}
   result['completed_profile_requests']=len(q.get('requests',[]))
print(json.dumps(result))
'''.replace('@WATCH@',repr(watch)).replace('@RUNTIME@',repr(runtime)).replace('@KIND@',repr(kind)).replace('@FORMAL_UID@',repr(formal['allocation_uid']))
result=jump(code)
start=time.monotonic()
try:
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as response:
        raw=response.read(65537)
        if len(raw)>65536:raise RuntimeError('bounded Windows formal health exceeded')
        health=json.loads(raw)
    result['Windows_health']=dict(status=response.status,loaded=health.get('loaded'),context=health.get('max_context'),seconds=time.monotonic()-start)
except OSError as error:
    result['Windows_health']=dict(phase='failed',error=type(error).__name__+': '+str(error)[:500])
result.update(formal=dict(uid=f['job_uid'],phase=f['phase'],observed_unix=f['unix']),
    optimization=dict(uid=o['job_uid'],phase=o['phase'],observed_unix=o['unix'],runtime=kind),
    pipeline={key:pipeline.get(key) for key in ['phase','profiler_phase','profiler_request','profiler_state','profiler_requests','error','observed_unix']})
path=BASE/('xolotl-progress-v175-'+str(time.time_ns())+'.json')
with path.open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
(BASE/'xolotl-progress-v175-latest-path.json').write_text(json.dumps(dict(file=path.name,unix=time.time()))+'\n')
print(json.dumps(result,indent=2))
