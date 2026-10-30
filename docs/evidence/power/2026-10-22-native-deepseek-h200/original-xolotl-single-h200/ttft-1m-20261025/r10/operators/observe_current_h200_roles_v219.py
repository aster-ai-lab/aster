"""Fresh C13/C12 identity, Windows health and current bounded model-comparison progress."""
import json
import time
import urllib.request
from xolotl_trial_common import BASE,jump,observe,owned
from xolotl_recovery_common_v217 import observe as observe_formal

formal=observe_formal();optimization=observe()
if formal['job_uid']!='e808d0b3-65c6-4a48-b121-220ff6fc5eb2' or optimization['job_uid']!='b37b7201-2c9c-45a2-ae1f-e522319324ca' or formal['job_uid']==optimization['job_uid']:raise RuntimeError('exact independent C13/C12 roles differ')
result=dict(unix=time.time(),formal=dict(uid=formal['job_uid'],phase=formal['phase']),optimization=dict(uid=optimization['job_uid'],phase=optimization['phase']),inference_sent=False,service_changed=False)
start=time.monotonic()
try:
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as response:
        raw=response.read(65537)
        if len(raw)>65536:raise RuntimeError('bounded formal Windows health exceeded')
        health=json.loads(raw)
    result['Windows_health']=dict(status=response.status,loaded=health.get('loaded'),context=health.get('max_context'),seconds=time.monotonic()-start)
except OSError as error:result['Windows_health']=dict(phase='failed',error=type(error).__name__+': '+str(error)[:500])
cp=BASE/'xolotl-topk-native-model-v213r1-controller.json'
if not cp.exists():cp=BASE/'xolotl-topk-native-model-v213-controller.json'
if cp.exists():
    controller=json.loads(cp.read_text());profile=controller.get('profile')
    if profile not in ['control_a','topk','control_b']:raise RuntimeError('closed native model profile differs')
    result['pipeline']={k:controller.get(k) for k in ['phase','profile','client_phase','client_tests','suite_phase','suite_stage','suite_children','completed_profiles','error','observed_unix']}
    intent=BASE/('xolotl-topk-native-v212-'+profile+'-intent.json')
    if intent.exists():
        service=json.loads(intent.read_text())
        if service['allocation_uid']!=optimization['job_uid']:raise RuntimeError('current experimental service allocation differs')
        code='''import json,time
from pathlib import Path
root=Path(@ROOT@)
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact active model profile root differs')
out=dict(unix=time.time())
p=root/'service-longctx-v1-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>65536:raise RuntimeError('bounded active model receipt exceeded')
 q=json.loads(p.read_text());out['service']={k:q.get(k) for k in ['phase','gpu_uuid','binary_sha256','context_tokens','error','lease_ends_unix']}
p=root/'engine.log'
if p.exists():
 if p.is_symlink() or p.stat().st_size>8*1024**2:raise RuntimeError('bounded exact active engine log exceeded')
 lines=p.read_text(errors='replace').splitlines()
 out['prefill_progress']=[line for line in lines if line.startswith('aster prefill phase: phase=progress total=1047544 ')][-3:]
 out['prompt_phases']=[line for line in lines if line.startswith('serve: prompt phases:')][-2:]
 out['decode_phases']=[line for line in lines if line.startswith(('serve: DSpark','serve: 1024 tokens','serve: pass split'))][-6:]
print(json.dumps(out))
'''.replace('@ROOT@',repr(service['root']))
        result['runtime']=jump(code)
        if optimization['phase']=='Running' and optimization.get('resource-result.json',{}).get('phase')=='passed':
            code='''import json,os
from pathlib import Path
binary=@BINARY@;expected=@MODE@;found=[]
for entry in Path('/proc').iterdir():
 if not entry.name.isdecimal():continue
 try:
  if entry.stat().st_uid!=10250 or os.readlink(entry/'exe')!=binary:continue
  raw=(entry/'environ').read_bytes()
  if len(raw)>1048576:raise RuntimeError('bounded exact native environment exceeded')
  selected=[part.removeprefix(b'ASTER_XQ_TOPK35=').decode() for part in raw.split(b'\\0') if part.startswith(b'ASTER_XQ_TOPK35=')]
  if selected!=[expected]:raise RuntimeError('exact native Top-K mode not propagated')
  stat=(entry/'stat').read_text().split(') ',1)[1].split();found.append(dict(pid=int(entry.name),start_ticks=int(stat[19]),mode=selected[0],namespace='optimization pod',flag_verified=True))
 except (FileNotFoundError,PermissionError,ProcessLookupError):continue
if len(found)>1:raise RuntimeError('multiple exact current native model processes observed')
print(json.dumps(dict(native=found,expected_mode=expected,all_environment_values_published=False)))
'''.replace('@BINARY@',repr(service['plan']['binary'])).replace('@MODE@',repr('1' if profile=='topk' else '0'))
            result['native_flag_observation']=owned(code)
proof=optimization.get('resource-result.json',{})
if proof.get('phase')=='passed':
    if len(proof['gpus'])!=1 or proof['gpus'][0]['uuid']!='GPU-099e8da4-8df0-c431-9e67-eb721e59beee':raise RuntimeError('exact same physical optimization H200 differs')
    result['optimization'].update(gpu_uuid=proof['gpus'][0]['uuid'],lease_ends_unix=proof['leaseEndsUnix'])
path=BASE/('xolotl-current-role-progress-v219-'+str(time.time_ns())+'.json')
with path.open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
(BASE/'xolotl-current-role-progress-v219-latest-path.json').write_text(json.dumps(dict(file=path.name,unix=time.time()))+'\n')
print(json.dumps(result,indent=2))
