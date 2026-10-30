"""Prepare exact full-input profile now; launch only after the finite API comparison and native link."""
import argparse
import ast
import base64
import hashlib
import json
import os
from pathlib import Path
import time
from xolotl_trial_common import BASE, jump, lease, observe, owned

parser=argparse.ArgumentParser();parser.add_argument('action',choices=['prepare','launch','observe']);action=parser.parse_args().action
pp=BASE/'xolotl-ncu-actual1m-v172r1-plan.json';ip=BASE/'xolotl-ncu-actual1m-v172r1-intent.json'
if action=='prepare':
    if pp.exists():raise RuntimeError('exact actual1M profiler plan exists; launch or observe it')
    service=json.loads((BASE/'xolotl-literal-service-v149-literal-intent.json').read_text())
    actual=json.loads((BASE/'xolotl-native-actual1m-input-v169-latest.json').read_text())
    ids=json.loads((BASE/'xolotl-native-actual1m-input-v169-intent.json').read_text())
    api=json.loads((BASE/'xolotl-api-literal-v148-intent.json').read_text())
    expected=next(row for row in service['files'] if row['file']=='server-config.json')
    code='''import hashlib,json
from pathlib import Path
r=@R@;e=@E@;api=@API@;p=Path(r['root'])/'server-config.json';src=Path(api['root'])
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=e['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=e['sha256']:raise RuntimeError('exact actual API native config differs')
manifest=json.loads((src/'public-manifest.json').read_text())
if len(manifest['files'])>512:raise RuntimeError('bounded exact public API manifest exceeded')
print(json.dumps(dict(config=json.loads(p.read_text()),public_inputs=[dict(path=str(src/row['file']),bytes=row['bytes'],sha256=row['sha256']) for row in manifest['files']])))
'''.replace('@R@',repr(service)).replace('@E@',repr(expected)).replace('@API@',repr(api))
    source=jump(code)
    config=source['config'];args=list(config['args'])
    raw=(BASE/'api-adapter-v148/serve_ds41.py').read_bytes()
    if hashlib.sha256(raw).hexdigest()!='6fd2a8b6135fc5e676933f8c729f33dc2bba5849d721cab6d28b6a68a9d43954':raise RuntimeError('exact effective API overlay differs')
    tree=ast.parse(raw)
    functions=[node for node in tree.body if isinstance(node,ast.FunctionDef) and node.name=='ds41_parallel_args']
    maxima=[node for node in tree.body if isinstance(node,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='DS41_PARALLEL_MAX' for t in node.targets)]
    if len(functions)!=1 or len(maxima)!=1:raise RuntimeError('exact API native-slot argument authority differs')
    namespace={'os':os,'DS41_PARALLEL_MAX':ast.literal_eval(maxima[0].value)}
    exec(compile(ast.Module(body=functions,type_ignores=[]),'pinned-effective-API-native-slots','exec'),namespace)
    args+=namespace['ds41_parallel_args'](config,args)
    if args.count('--batch')!=1 or args[args.index('--batch')+1]!='2' or args[args.index('--ctx')+1]!='1048576' or args[args.index('--prefill-chunk')+1]!='512':raise RuntimeError('actual effective1M/S512/batch2 profile differs')
    if actual['phase']!='passed' or actual['prompt_tokens']!=1047544 or not actual['full_body_unchanged']:raise RuntimeError('exact canonical real1M input preparation incomplete')
    baseline=json.loads((BASE/'qualified-xolotl-single-h200-baseline.json').read_text())['plan']
    plan={key:baseline[key] for key in ['model','rank','sidecar','engram']}
    env={key:value for key,value in service['plan']['engine_env'].items() if key not in ['ASTER_XQ_LITERAL_SCAN','ASTER_XQ_EXACT_TOKEN_CACHE']}
    plan.update(schema='aster-ncu-actual1m-v172r1',context_tokens=1048576,prompt_tokens=1047544,profile_output_tokens=128,
        native_args=args,engine_env=env,expected_gpu_uuid=service['gpu_uuid'],allocation_uid=service['allocation_uid'],
        input_root=ids['root'],public_api_root=api['root'],lease_ends_unix=service['plan']['lease_ends_unix'],
        prior_service_root=service['root'],diagnostic_only=True,full1024_completion_gate_substituted=False,
        inputs=source['public_inputs']+[dict(path=ids['root']+'/'+row['file'],bytes=row['bytes'],sha256=row['sha256']) for row in actual['files']])
    with pp.open('x') as output:output.write(json.dumps(plan,indent=2)+'\n')
    sealed=(BASE/'xolotl_sealed_bootstrap_v139.py').read_text()
    old="LABELS={'phase-simd-v139-control_a','phase-simd-v139-simd','phase-simd-v139-control_b'}"
    if sealed.count(old)!=1:raise RuntimeError('exact bounded sealed profiler label set differs')
    with (BASE/'xolotl_sealed_bootstrap_v172r1.py').open('x') as output:output.write(sealed.replace(old,"LABELS={'ncu-actual1m-v172r1'}"))
    print(json.dumps(dict(phase='prepared',actual_input_tokens=1047544,profile_output_tokens=128,native_slots=2,gpu_executed=False)))
elif action=='launch':
    if ip.exists():raise RuntimeError('exact hardware profiler has a handle; observe it')
    plan=json.loads(pp.read_text());o=observe()
    comparison=json.loads((BASE/'xolotl-literal-api-v151-controller.json').read_text())
    link=json.loads((BASE/'xolotl-native-serve-profile-v168-latest.json').read_text())
    q=link['native-link-result.json']
    if comparison['phase']!='comparison_completed' or link['exact_link_process_alive'] or q['phase']!='linked' or not q['control_binary_byte_equal'] or q['invalid_modes_refused']!=8:
        raise RuntimeError('exact API comparison and controlled profiler link must finish first')
    if o['phase']!='Running' or o['job_uid']!=plan['allocation_uid']:raise RuntimeError('exact independent optimization allocation differs')
    plan.update(binary=q['binary']['path'],binary_sha256=q['binary']['sha256'])
    files={'worker.py':(BASE/'xolotl_ncu_actual1m_worker_v171r1.frozen.py').read_bytes(),
        'aster_ncu_metrics_v171.py':(BASE/'aster_ncu_metrics_v171.py').read_bytes(),
        'test_ncu_metrics_v171.py':(BASE/'test_ncu_metrics_v171.py').read_bytes(),
        'aster_ncu_query_v178.py':(BASE/'aster_ncu_query_v178.py').read_bytes(),
        'test_ncu_query_v178.py':(BASE/'test_ncu_query_v178.py').read_bytes(),
        'xolotl_fullwindow_long_fixture_v54.py':(BASE/'xolotl_fullwindow_long_fixture_v54.py').read_bytes(),
        'plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
    keys=[p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt')]
    if not keys or any(not key or key in raw for key in keys for raw in files.values()):raise RuntimeError('actual credential negative control refused profiler payload')
    record=dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/native-hardware-actual1m-v172r1',
        allocation_uid=o['job_uid'],gpu_uuid=plan['expected_gpu_uuid'],binary_sha256=plan['binary_sha256'],
        actual_input_tokens=1047544,profile_output_tokens=128,diagnostic_only=True,credentials_copied=False,api_started=False,
        files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
    code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;p=@P@;root=Path(r['root']);data=@DATA@
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive actual1M profiler stage differs')
old=json.loads((Path(p['prior_service_root'])/'service-longctx-v1-result.json').read_text())
if old['phase']!='stopped' or not old.get('api_reaped'):raise RuntimeError('exact prior API/model must be stopped and reaped')
for name,size in [(p['model'],162954903552),(p['rank'],122880)]:
 path=Path(name)
 if not path.is_file() or path.stat().st_size!=size:raise RuntimeError('exact native profiling model/rank size differs')
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if active:raise RuntimeError('exact optimization GPU must be idle before profiler')
proof=json.loads((root.parent/'resource-result.json').read_text())
if proof['phase']!='passed' or proof['gpus'][0]['uuid']!=r['gpu_uuid'] or time.time()+4500+120>proof['leaseEndsUnix']:raise RuntimeError('finite actual1M hardware profiler lease insufficient')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact frozen hardware profiler input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'numeric-parser-tests.log').open('xb') as log:
 result=subprocess.run(['python3','-m','unittest','-v','test_ncu_metrics_v171','test_ncu_query_v178'],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdout=log,stderr=log,timeout=30)
if result.returncode:raise RuntimeError('hermetic numeric profiler parser gates failed before GPU launch')
with (root/'worker.log').open('xb') as log:
 process=subprocess.Popen(['timeout','--signal=TERM','--kill-after=30','4500','python3','-u',str(root/'worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
handle=dict(pid=process.pid,start_ticks=int(Path('/proc/'+str(process.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=4500,namespace='optimization pod')
(root/'process.json').write_text(json.dumps(handle,indent=2)+'\\n');print(json.dumps(handle))
'''.replace('@R@',repr(record)).replace('@P@',repr(plan)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
    compile(code,'launch-owned-actual1m-hardware-profiler-v172r1','exec')
    with ip.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
    record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
    try:
        from xolotl_sealed_bootstrap_v172r1 import sealed_owned
        record.update(process=sealed_owned(code,'ncu-actual1m-v172r1'),phase='launched')
    except BaseException:
        record['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(record,indent=2)+'\n');raise
    ip.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(phase=record['phase'],process=record['process'],actual_input_tokens=1047544)))
else:
    r=json.loads(ip.read_text());o=observe()
    if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact hardware profiler allocation differs')
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out=dict(unix=time.time())
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact hardware profiler owner differs')
process=r.get('process') or json.loads((root/'process.json').read_text());p=Path('/proc/'+str(process['pid'])+'/stat')
out['exact_worker_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==process['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['result.json','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>2*1024**2:raise RuntimeError('bounded hardware profiler receipt exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-4096:]
q=out.get('result.json',{})
if q.get('phase')=='failed':
 for name in ['cpu-protocol-ncu.log','metric-query.stdout.log','metric-query.stderr.log','private-ncu.log','engine.stderr.log']:
  p=root/name
  if p.exists() and p.stat().st_size<8*1024**2:out[name]=p.read_text(errors='replace')[-4096:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
    result=owned(code);(BASE/'xolotl-ncu-actual1m-v172r1-latest.json').write_text(json.dumps(result,indent=2)+'\n')
    q=result.get('result.json',{})
    print(json.dumps(dict(phase=q.get('phase'),alive=result['exact_worker_alive'],current=q.get('current_request'),state=q.get('current_state'),
        requests=len(q.get('requests',[])),hardware_counters=q.get('hardware_counters_collected'),families=q.get('kernel_family_counts'),
        native_gone=q.get('native_target_gone'),engine_reaped=q.get('engine_reaped'),error=q.get('error')),indent=2))
