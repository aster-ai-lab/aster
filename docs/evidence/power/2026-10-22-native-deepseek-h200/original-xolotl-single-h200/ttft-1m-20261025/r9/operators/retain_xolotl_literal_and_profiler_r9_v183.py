"""Append the completed canonical API comparison, exact CPU footprint and profiler preparation."""
import ast
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess
import time
from aster_ncu_metrics_v171 import METRICS
from aster_ncu_query_v178 import confirmed_metrics

B=Path(__file__).resolve().parent
W=Path('D:/code/aster/.wt-h200-native-integration')
E=W/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200'
D=E/'ttft-1m-20261025/r9'
def read(name):return json.loads((B/name).read_text(encoding='utf-8-sig'))

matrix=read('xolotl-literal-api-v151-controller.json')
if D.exists() or subprocess.check_output(['git','status','--porcelain'],cwd=W):raise RuntimeError('R9 requires a clean integration worktree and a new immutable revision')
if matrix['phase']!='comparison_completed' or matrix['candidate_promoted'] or matrix['literal_identity_gate_passed'] or matrix['control_identity_gate_passed']:
    raise RuntimeError('exact completed API comparison and preserved failed identity gates required')
suites={name:read('xolotl-literal-fullwindow-v150-'+name+'-latest.json') for name in ['control','literal']}
for name,result in suites.items():
    q=result['fullwindow/request-result.json']
    if result['exact_client_alive'] or len(q['tests'])!=20 or q['phase']!='failed' or q.get('error')!='ValueError: full-window long-output identity differs' or not q['exact_restored_long_output_matched'] or len(q['levels'])!=1 or q['levels'][0]['concurrency']!=1:
        raise RuntimeError('exact cold/restore/c1 proof and failed C2 gate required: '+name)
    if not all(t['usage']['prompt_tokens']==1047544 and t['usage']['completion_tokens']==1024 and t['done'] and t['finish_reason']=='length' for t in q['tests'] if t['name'].startswith(('cold-full1m','restored-full1m','c1-','c2-'))):
        raise RuntimeError('actual full-input/output completion count differs')
basic=read('xolotl-literal-service-v149-literal-qualification-latest.json')['result']
extended=read('xolotl-literal-extended-v149-literal-latest.json')['result']
control_extended=read('xolotl-phase-simd-extended-v139-control_b-latest.json')['result']
cpu=read('xolotl-serve-profile-cpu-v167-latest.json')
footprint=read('xolotl-engram-hash-footprint-v181-latest.json')
query=read('xolotl-ncu-suffix-query-v177.json')
if basic['phase']!='passed' or len(basic['tests'])!=28 or extended['phase']!='passed' or len(extended['tests'])!=12 or not extended['exact_long_output_reference_matched']:
    raise RuntimeError('exact40 ordinary literal API gates required')
if cpu['exact_cpu_process_alive'] or cpu['build-result.json']['phase']!='compiled' or [row['result']['contracts'] for row in cpu['build-result.json']['tests']]!=[20015,20015]:
    raise RuntimeError('exact compiled CPU profiler lifecycle/sanitizer proof required')
if footprint['alive'] or footprint['result.json']['phase']!='passed':raise RuntimeError('exact CPU hash footprint must finish')
supported=confirmed_metrics(query['stdout'])
keys=list({p.read_bytes().strip() for p in B.glob('access-private-*/api-key.txt') if len(p.read_bytes().strip())>=24})
if not keys:raise RuntimeError('actual credential negative controls absent')
mp=E/'manifest.json';original=mp.read_bytes();manifest=json.loads(original);old=list(manifest['files'])
if len(old)!=2558:raise RuntimeError('exact prior R8 manifest row count differs')
for row in old:
    raw=(E/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('historical evidence differs: '+row['file'])
D.mkdir(parents=True);files=[]
def put(raw,name,source=None):
    if len(raw)>2*1024**2 or any(key in raw for key in keys):raise RuntimeError('bounded actual credential publication gate refused: '+name)
    if name.endswith('.py'):ast.parse(raw,filename=name)
    if name.endswith('.json'):json.loads(raw)
    p=D/name;p.parent.mkdir(parents=True,exist_ok=True)
    with p.open('xb') as output:output.write(raw)
    files.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),source=source))
def copy(name,target):put((B/name).read_bytes(),target,name)
def data(value,name):put((json.dumps(value,indent=2,ensure_ascii=False)+'\n').encode(),name)

receipts=['xolotl-literal-api-v151-controller.json','xolotl-literal-service-v149-literal-intent.json',
    'xolotl-literal-service-v149-literal-qualification-intent.json','xolotl-literal-service-v149-literal-qualification-latest.json',
    'xolotl-literal-extended-v149-literal-intent.json','xolotl-literal-extended-v149-literal-latest.json',
    'xolotl-literal-fullwindow-v150-control-intent.json','xolotl-literal-fullwindow-v150-control-latest.json',
    'xolotl-literal-fullwindow-v150-literal-intent.json','xolotl-literal-fullwindow-v150-literal-latest.json',
    'xolotl-phase-simd-extended-v139-control_b-latest.json',
    'xolotl-serve-profile-cpu-v167-prepared.json','xolotl-serve-profile-cpu-v167-intent.json','xolotl-serve-profile-cpu-v167-latest.json',
    'xolotl-native-serve-profile-v168-plan.json','xolotl-native-actual1m-input-v169-intent.json','xolotl-native-actual1m-input-v169-latest.json',
    'xolotl-ncu-capture-cli-v170.json','xolotl-ncu-actual1m-v172-plan.json','xolotl-ncu-actual1m-v172r1-plan.json',
    'xolotl-fullwindow-hardware-v173-controller.json','xolotl-hardware-controller-v173-intervention-v177.json',
    'xolotl-ncu-preflight-v176.json','xolotl-bounded-ncu-query-v178-prepared.json',
    'xolotl-engram-hash-inputs-v179.json','xolotl-engram-hash-footprint-v181-intent.json','xolotl-engram-hash-footprint-v181-latest.json',
    'xolotl-literal-runtime-v182.json']
optional=['xolotl-native-serve-profile-v168-intent.json','xolotl-native-serve-profile-v168-latest.json',
    'xolotl-ncu-actual1m-v172r1-intent.json','xolotl-ncu-actual1m-v172r1-latest.json',
    'xolotl-fullwindow-hardware-v173r1-controller.json']
for name in receipts+[name for name in optional if (B/name).exists()]:copy(name,'receipts/'+name)
latest=read('xolotl-progress-v175-latest-path.json');copy(latest['file'],'receipts/fresh-role-observation.json')
operators=['prepare_serve_decode_profile_v166.py','prepare_serve_profile_cpu_v167.py','serve_decode_profile_cpu_worker_v167.frozen.py','manage_serve_profile_cpu_v167.py',
    'serve_profile_native_link_worker_v168.frozen.py','manage_serve_profile_native_v168.py','xolotl_sealed_bootstrap_v168.py',
    'prepare_native_actual1m_ids_v169.py','inspect_ncu_capture_cli_v170.py','aster_ncu_metrics_v171.py','test_ncu_metrics_v171.py',
    'xolotl_ncu_actual1m_worker_v171.frozen.py','xolotl_ncu_actual1m_worker_v171r1.frozen.py',
    'manage_ncu_actual1m_v172.py','manage_ncu_actual1m_v172r1.py','xolotl_sealed_bootstrap_v172.py','xolotl_sealed_bootstrap_v172r1.py',
    'run_fullwindow_then_hardware_profile_v173.frozen.py','run_fullwindow_then_hardware_profile_v173r1.frozen.py',
    'observe_hardware_pipeline_v175.py','preflight_installed_ncu_protocol_v176.py','inspect_ncu_metric_suffix_v177.py',
    'aster_ncu_query_v178.py','test_ncu_query_v178.py','prepare_bounded_ncu_query_v178.py',
    'inspect_engram_hash_cpu_inputs_v179.py','engram_hash_footprint_v180.cpp','run_engram_hash_footprint_v181.py',
    'inspect_literal_runtime_activation_v182.py','collect_profiler_cpu_public_v184.py','retain_xolotl_literal_and_profiler_r9_v183.py']
for name in operators:copy(name,'operators/'+name)
for path in sorted((B/'serve-decode-profile-v166').iterdir()):
    if path.is_file() and path.suffix in ['.cpp','.hpp','.patch','.json']:copy(path.relative_to(B).as_posix(),'frozen-candidate/'+path.name)
copy('xolotl-trial-source-e9d0826d71c5/source/LICENSE','frozen-candidate/upstream-LICENSE')
collection=read('xolotl-profiler-cpu-public-v184.json')
for row in collection['files']:
    raw=(B/'profiler-cpu-public-v184'/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact public CPU diagnostic bytes differ')
    put(raw,'public-diagnostics/'+row['file'],'profiler-cpu-public-v184/'+row['file'])
copy('xolotl-profiler-cpu-public-v184.json','public-diagnostics/collection-manifest.json')
selected=[row for row in csv.reader(io.StringIO(query['stdout'])) if row and row[0] in METRICS]
data(dict(query_returncode=query['query_returncode'],query_stdout_bytes=query['stdout_bytes'],query_stdout_sha256=query['stdout_sha256'],
    confirmed_metrics=supported,installed_H200_supported_rows=[dict(metric=row[0],unit=row[2],supported=row[4]) for row in selected],
    GPU_math_executed=False,model_touched=False,hardware_counters_collected=False),'installed-counter-support.json')
a=suites['control']['fullwindow/request-result.json'];b=suites['literal']['fullwindow/request-result.json']
summary=dict(schema='aster-literal-api-real1m-profiler-r9',unix=time.time(),same_GPU='GPU-099e8da4-8df0-c431-9e67-eb721e59beee',
    same_native_sha256='99f3eee8c7eeee54c304120db50e8e632a7c001f9ac90fadf0983040fef92c25',
    ordinary_literal_API_requests=40,ordinary_full_output_and_native_tool_passed=True,
    ordinary_control_8x1024=control_extended['long_summary'],ordinary_literal_8x1024=extended['long_summary'],
    fullwindow_calls_per_profile=20,actual1m_calls_per_profile=18,actual_input_tokens=1047544,output_tokens=1024,
    control=dict(cold=a['tests'][1],restored=a['tests'][3],c1=a['levels'][0]),
    literal=dict(cold=b['tests'][1],restored=b['tests'][3],c1=b['levels'][0]),
    C2_full_output_identity_passed=False,formal_promoted=False,material_GPU_speed_gain_qualified=False,
    native_runtime_input_digest_captured=False,cold_prefill_attribution_controlled=False,
    CPU_hash_footprint=footprint['result.json']['result'],cache_capacity_changed=False,
    profiler_CPU_contracts_per_mode=20015,profiler_requested_metrics=supported,
    actual_model_hardware_counters_collected=False,hardware_profile_result_pending=True,
    full1024_gate_substituted=False,goal_complete=False,single_request300_met=False)
data(summary,'round-summary.json')
outcome='''# Canonical real1M literal API comparison and hardware profiler preparation (R9)

The same H200099e/native99f3/S512/1M/batch2 configuration completes the
baseline/literal API comparison. The literal overlay passes28 standard and12
native-tool/1024-output gates. Short-prompt8x1024 API-wall throughput is
75.503822 versus75.789067 token/s, an observed0.38% difference.

Each fullwindow profile completes20 calls, including18 actual1047544-token
inputs with1024 output tokens. Cold, isolated-restored and all8 sequential
full-output SHA values match. Sequential8-call API-wall throughput is
23.040565 versus23.101921 token/s; TTFT P50 is7.889305 versus7.884743 seconds.
These differences do not qualify a material end-to-end improvement. The CPU
encoder's earlier82% isolated timing reduction is not an API speed claim.
Both profiles'8 concurrency2 calls structurally finish and retrieve all3
records, but complete-output identity fails. Their failed acceptance remains
visible and no candidate is promoted. Baseline cold prefill overlapped C11
recovery; cold-time differences are not a controlled attribution.

Exact live API PID/start/UID, public startup flags, four source hashes and
closed disabled-helper warning counts were inspected without an inference
request or exposing other environment variables. Flags1/1 and pinned files
are present, with no declared disabled-helper warning. This is not an
in-process scanner-hit trace or a native runtime input-ID digest capture.

The canonical sender fixture has1047544 protected API IDs and digest b442a593.
Its private u32/CSV files are not published; exact bytes/hashes and generator
source are retained. Default-off native fourth-GEN decode hooks pass20015
lifecycle contracts in each of release and ASAN/UBSAN modes. The original
host object reproduces byte-for-byte, and all original GPU archives remain
in the subsequent controlled-link plan. The planned diagnostic keeps three
unprofiled128-ID controls before capturing the fourth decode; it does not
replace the1024-output API gates or establish actual1M API throughput.

Installed Nsight Compute2024.3 CPU stdin/stdout smoke passes. Its all-metric
query ignores the intended base filter in all mode and exceeds the declared
read bound. That refusal is retained. Only the waiting local observer was
stopped; no model or formal service was signalled. A suffix-only CSV query
then confirms all8 exact metrics on NVIDIA H200 in67053 bytes. Twelve
hermetic numeric/query parser methods pass before the revised capture launch.
Clock/cache controls remain none; raw profiler reports remain private. The
new actual-model hardware-counter run is pending this snapshot.

A CPU-only executable links the exact original Engram host object and uses
its public setup_hash/hash API on the same canonical IDs. It opens no row
table and executes no model forward or GPU work. The50,282,112 references
map to12,953,683 unique layer/row keys:6,476,814 and6,476,869 by layer.
Their264-byte raw rows total3,419,772,312 bytes, excluding map overhead.
This supports testing16M cache rows against the present2M fill-only cache;
capacity, hit ratio and actual inference speed have not been changed/tested.

## Commands and scope

- run_literal_api_comparison_v151.frozen.py: completed same-native API A/B;
  manage_literal_fullwindow_suite_v150.py observe --profile control|literal.
- manage_serve_profile_cpu_v167.py: exact host object and20015x2 lifecycle proof.
- preflight_installed_ncu_protocol_v176.py: retained overbroad metric refusal;
  inspect_ncu_metric_suffix_v177.py and12 parser methods confirm the revision.
- run_fullwindow_then_hardware_profile_v173r1.frozen.py: after completed API
  calls and exact idle drain, link168 then bounded actual1M profiler172r1.
- run_engram_hash_footprint_v181.py: one finite isolated jump-host CPU footprint.
- observe_hardware_pipeline_v175.py: exact C11/C10 resource identity and direct
  Windows formal health, without inference or clock/cache policy changes.

## Explicitly not claimed

No material API improvement, fixed C2 determinism,300/1000 single-request
achievement, goal completion, actual-model hardware counters, kernel speedup,
larger cache performance, fresh WorkBuddy GUI acceptance, fresh formal C11
actual1M execution, new Rust workspace CI/fmt result, raw profiler report,
weight, compiled binary, API credential or private client state is claimed
or published. The formal C11 VIP/key/configuration remains unchanged.
'''
put(outcome.encode(),'OUTCOME.md')
data(dict(schema='aster-literal-profiler-r9-manifest',unix=time.time(),files=files),'manifest.json')
known={row['file'] for row in old}
for path in sorted(D.rglob('*')):
    if path.is_file():
        name=path.relative_to(E).as_posix();raw=path.read_bytes()
        if name in known:raise RuntimeError('immutable evidence collision')
        manifest['files'].append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
if manifest['files'][:len(old)]!=old:raise RuntimeError('prior manifest order changed')
manifest.update(latest_literal_and_hardware_preparation_revision='ttft-1m-20261025/r9',goal_complete=False)
newline='\r\n' if b'\r\n' in original else '\n'
mp.write_bytes((json.dumps(manifest,indent=2)+'\n').replace('\n',newline).encode())
p=W/'docs/source-manifest.json';source=json.loads(p.read_text());entry=source['original_xolotl_h200_20261024'];relative=D.relative_to(W).as_posix()
entry['latest_exact_literal_frontend'].update(evidence=relative,GPU_ordinary_requests=40,actual1m_comparison_completed=True,
    real1m_calls_per_profile=18,full_cold_restore_and_c1_output_text_passed=True,complete_C2_output_identity_passed=False,
    gpu_api_improvement_qualified=False,production_promoted=False)
entry['latest_exact_hardware_profiler']=dict(evidence=relative,CPU_contracts_per_mode=20015,requested_exact_H200_metrics=8,
    query_mode='suffix CSV',actual_model_hardware_counters_collected=False,full1024_gate_substituted=False,formal_changed=False)
entry['latest_canonical_engram_footprint']=dict(evidence=relative,prompt_tokens=1047544,unique_rows=12953683,raw_row_bytes=3419772312,
    hash_only=True,cache_capacity_changed=False,speed_improvement_claimed=False)
roles=entry['current_serving_optimization_roles'];roles['publication']['evidence_revision']='ttft-1m-20261025/r9'
roles.update(unix=time.time(),goal_met=False,next_action='Observe exact V173r1/V168/V172r1 handles on C10 for actual1M hardware counters. Keep failed C2 gate and formal C11 unchanged; test16M Engram cache only in a subsequent isolated controlled campaign.')
roles['optimization'].update(latest_evidence=relative,current_profile='Completed literal API A/B: no material throughput/TTFT improvement; actual1M native counter profile pending',
    full1m_candidate_qualified=False,production_promoted=False,concurrent_identity_fixed=False,ordinary_decode_gain_proven=False)
p.write_text(json.dumps(source,indent=2,ensure_ascii=False)+'\n',encoding='utf-8',newline='\n')
p=W/'.gitattributes';text=p.read_text();rule='/docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200/ttft-1m-20261025/r9/frozen-candidate/*.patch -text whitespace=-blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol\n'
p.write_text(text.rstrip()+'\n'+rule,encoding='utf-8',newline='\n')
print(json.dumps(dict(phase='retained',revision='r9',new_files=sum(p.is_file() for p in D.rglob('*')),prior_entries_preserved=len(old),
    literal_ordinary40_passed=True,actual1m_calls_per_profile=18,material_speed_gain=False,C2_identity_passed=False,formal_changed=False,goal_complete=False)))
