"""Prepare now; launch only after the exact ongoing model/client are drained; observe exact build."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease, observe, owned

parser=argparse.ArgumentParser();parser.add_argument('action',choices=['prepare','launch','observe']);action=parser.parse_args().action
pp=BASE/'xolotl-native-serve-profile-v168-plan.json';ip=BASE/'xolotl-native-serve-profile-v168-intent.json'
if action=='prepare':
    if pp.exists():raise RuntimeError('exact profile link plan exists; launch or observe it')
    parent=json.loads((BASE/'xolotl-native-phase-simd-v138-plan.json').read_text())
    original=json.loads((BASE/'xolotl-prefill-phase-cpu-v134-latest.json').read_text())['build-result.json']['candidate_object']
    proof=json.loads((BASE/'xolotl-serve-profile-cpu-v167-latest.json').read_text())
    q=proof['build-result.json']
    if proof['exact_cpu_process_alive'] or q['phase']!='compiled':raise RuntimeError('exact profiler CPU build must finish')
    link=list(parent['control_link'])
    if link.count(parent['original_generator'])!=1:raise RuntimeError('exact traced native substitution differs')
    link[link.index(parent['original_generator'])]=original['path']
    service=json.loads((BASE/'xolotl-literal-service-v149-literal-intent.json').read_text())
    formal=json.loads((BASE/'xolotl-active-serving-reservation.json').read_text())
    plan=dict(schema='aster-native-serve-profiler-v168',link_cwd=parent['link_cwd'],control_link=link,
        original_generator=original['path'],control_binary_sha256='99f3eee8c7eeee54c304120db50e8e632a7c001f9ac90fadf0983040fef92c25',
        profile_cpu_root=json.loads((BASE/'xolotl-serve-profile-cpu-v167-intent.json').read_text())['root'],
        inputs=parent['inputs']+[q['candidate_object']],prior_service_root=service['root'],
        allocation_uid=service['allocation_uid'],gpu_uuid=service['gpu_uuid'],formal_gpu_uuid=formal['gpu_uuid'])
    with pp.open('x') as output:output.write(json.dumps(plan,indent=2)+'\n')
    sealed=(BASE/'xolotl_sealed_bootstrap_v139.py').read_text()
    before="LABELS={'phase-simd-v139-control_a','phase-simd-v139-simd','phase-simd-v139-control_b'}"
    if sealed.count(before)!=1:raise RuntimeError('exact closed transport label set differs')
    sealed=sealed.replace(before,"LABELS={'serve-profiler-link-v168'}")
    with (BASE/'xolotl_sealed_bootstrap_v168.py').open('x') as output:output.write(sealed)
    print(json.dumps(dict(phase='prepared',gpu_executed=False,prior_model_must_be_drained=True)))
elif action=='launch':
    if ip.exists():raise RuntimeError('exact link handle exists; inspect it')
    plan=json.loads(pp.read_text());o=observe()
    matrix=json.loads((BASE/'xolotl-literal-api-v151-controller.json').read_text())
    if o['job_uid']!=plan['allocation_uid'] or o['phase']!='Running' or matrix['phase']!='comparison_completed':raise RuntimeError('exact finite prior comparison must complete before native link')
    code='''import json,subprocess
from pathlib import Path
p=@P@;q=json.loads((Path(p['prior_service_root'])/'service-longctx-v1-result.json').read_text())
if q['phase']!='stopped' or not q.get('api_reaped'):raise RuntimeError('exact literal service must be stopped/reaped before link')
r=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5)
if r.stdout.strip():raise RuntimeError('exact optimization GPU must be idle')
print(json.dumps(dict(prior_model_drained=True,gpu_idle=True)))
'''.replace('@P@',repr(plan))
    idle=owned(code)
    files={'native-link-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'worker.py':(BASE/'serve_profile_native_link_worker_v168.frozen.py').read_bytes()}
    record=dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/build-native-serve-profile-v168',allocation_uid=o['job_uid'],gpu_uuid=plan['gpu_uuid'],idle=idle,
        gpu_qualified=False,files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
    code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive exact profiling link root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact frozen profiling link input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','200','python3','-u',str(root/'worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=200,namespace='optimization pod')
(root/'process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
    compile(code,'launch-native-serve-profiler-link-v168','exec')
    with ip.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
    record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
    try:
        from xolotl_sealed_bootstrap_v168 import sealed_owned
        record.update(process=sealed_owned(code,'serve-profiler-link-v168'),phase='launched')
    except BaseException:
        record['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(record,indent=2)+'\n');raise
    ip.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(phase=record['phase'],process=record['process'])))
else:
    r=json.loads(ip.read_text());o=observe()
    if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact current profiling-link allocation differs')
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out=dict(unix=time.time())
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact profiling link root owner differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_link_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['native-link-result.json','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded profiling link observation exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-4096:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
    result=owned(code);(BASE/'xolotl-native-serve-profile-v168-latest.json').write_text(json.dumps(result,indent=2)+'\n')
    q=result.get('native-link-result.json',{})
    print(json.dumps(dict(phase=q.get('phase'),alive=result['exact_link_process_alive'],control_byte_equal=q.get('control_binary_byte_equal'),binary=q.get('binary'),error=q.get('error')),indent=2))
