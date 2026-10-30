"""Append immutable finite natural-prefill comparisons, rejected decoder and concurrent-path diagnostics."""
import ast
import hashlib
import json
import subprocess
import time
from pathlib import Path

B=Path('D:/code/aster/target/h200-native');W=Path('D:/code/aster/.wt-h200-native-integration')
E=W/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200'
D=E/'ttft-1m-20261025/r6'
if D.exists() or subprocess.check_output(['git','status','--porcelain'],cwd=W):
    raise RuntimeError('immutable R6 requires an exclusive clean integration worktree')


def read(name):return json.loads((B/name).read_text(encoding='utf-8-sig'))


directory=read('xolotl-engram-comparison-v71r1-controller.json')
decoder=read('xolotl-engram-lut-v78-controller.json')
repeat=read('xolotl-engram-local-repeat-v84-controller.json')
probe=read('xolotl-short-concurrency-v85-latest.json')
cpu=read('xolotl-engram-fp8-cpu-v72-latest.json')
build=read('xolotl-engram-fp8-build-v73-latest.json')
if (directory['phase']!='passed' or len(directory['profiles'])!=3
        or any(row['natural_tests']!=7 or row['extended_tests']!=12 for row in directory['profiles'])
        or decoder['phase']!='passed' or decoder['decoder_natural_tests']!=7 or decoder['decoder_extended_tests']!=12
        or repeat['phase']!='passed' or repeat['client_tests']!=7
        or probe['exact_client_alive'] or probe['result']['phase'] not in ['passed','failed']
        or cpu['phase']!='passed' or not cpu['exact_fp8_all_encodings_and_scales_passed']
        or build['phase']!='compiled' or not build['original_object_byte_equal'] or not build['default0_object_byte_equal']):
    raise RuntimeError('exact completed comparisons, CPU reproduction and terminal diagnostic required')
keys=[]
for path in B.glob('access-private-*/api-key.txt'):
    value=path.read_bytes().strip()
    if len(value)>=24 and value not in keys:keys.append(value)
if not keys:raise RuntimeError('actual credential negative controls missing')
manifest_path=E/'manifest.json';original=manifest_path.read_bytes();manifest=json.loads(original);old_rows=list(manifest['files'])
for row in old_rows:
    raw=(E/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('historical evidence changed: '+row['file'])
files=[];D.mkdir(parents=True)


def put(raw,name,source=None):
    if len(raw)>2*1024**2 or any(key in raw for key in keys):raise RuntimeError('bounded public credential gate refused: '+name)
    if name.endswith('.py'):ast.parse(raw,filename=name)
    path=D/name;path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as output:output.write(raw)
    files.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),source=source))


def copy(name,target):put((B/name).read_bytes(),target,name)


def data(value,name):put((json.dumps(value,indent=2,ensure_ascii=False)+'\n').encode(),name)


patterns=[
 'xolotl-natural-fixtures-v67-*.json','xolotl-engram-disk-v68-*.json','xolotl-engram-natural-v69-*.json',
 'xolotl-engram-extended-v70-*.json','xolotl-engram-transition-v71.json','xolotl-engram-comparison-v71*.json',
 'xolotl-engram-gpfs-b-*.json','xolotl-engram-fp8-cpu-v72-*.json','xolotl-engram-fp8-recipe-discovery-v72.json',
 'xolotl-engram-fp8-build-v73-*.json','xolotl-engram-fp8-link-v74-*.json','xolotl-engram-lut-v75-*.json',
 'xolotl-engram-lut-natural-v76-*.json','xolotl-engram-lut-extended-v77-*.json','xolotl-engram-lut-v78-controller.json',
 'xolotl-engram-local-repeat-v79-*.json','xolotl-engram-local-repeat-v80-*.json','xolotl-engram-local-repeat-v82-controller.json',
 'xolotl-engram-local-repeat-recovery-v83-*.json','xolotl-engram-local-repeat-v84-controller.json',
 'xolotl-short-concurrency-v85-*.json','xolotl-active-optimization-config-v85.json','xolotl-readiness-watch-v65-latest.json',
]
for name in sorted({path.name for pattern in patterns for path in B.glob(pattern)}):copy(name,'receipts/'+name)
operators=[
 'xolotl_natural_fixture_prepare_v67.frozen.py','stage_xolotl_natural_fixture_v67.frozen.py',
 'launch_xolotl_engram_disk_v68.frozen.py','manage_xolotl_engram_disk_v68.frozen.py',
 'xolotl_natural_prefill_worker_v69.frozen.py','manage_xolotl_engram_natural_v69.frozen.py',
 'manage_xolotl_engram_extended_v70.frozen.py','check_xolotl_engram_transition_v71.py',
 'run_xolotl_engram_comparison_v71.py','collect_xolotl_engram_numeric_v71.py','recover_xolotl_unstarted_gpfs_b_v71r1.py',
 'prepare_xolotl_engram_fp8_lut_v72.py','run_xolotl_engram_fp8_cpu_v72.py','build_xolotl_engram_fp8_cpu_v73.py',
 'launch_xolotl_engram_fp8_link_v74.py','launch_xolotl_engram_lut_v75.frozen.py','manage_xolotl_engram_lut_service_v75.frozen.py',
 'manage_xolotl_engram_lut_natural_v76.frozen.py','manage_xolotl_engram_lut_extended_v77.frozen.py','finish_xolotl_engram_and_lut_v78.py',
 'launch_xolotl_engram_local_repeat_v79.frozen.py','manage_xolotl_engram_local_repeat_service_v79.frozen.py',
 'manage_xolotl_engram_local_repeat_v80.frozen.py','xolotl_short_concurrency_probe_v81.frozen.py','repeat_xolotl_local_control_v82.py',
 'recover_xolotl_unstarted_local_repeat_v83.py','finish_xolotl_local_repeat_v84.py','manage_xolotl_short_concurrency_v85.py',
 'xolotl_trial_common.py','xolotl_serving_common.py','retain_xolotl_engram_comparison_r6.py',
]
for name in operators:copy(name,'operators/'+name)
for name in ['ds41_engram.original.cpp','ds41_engram.cpp','fp8_exact_test.cpp','engram-fp8-lut-v72.patch','source-manifest.json','native_link_worker_v74.py']:
    copy('engram-fp8-lut-v72/'+name,'frozen-candidates/engram-fp8-lut-v72/'+name)
for name in ['technical8244.txt','technical25671.txt']:copy('public-natural-fixtures-v67/'+name,'fixtures/'+name)
profiles=directory['profiles']
local=next(row for row in profiles if row['profile']=='local')
gains=[]
for index,case in enumerate(local['cases']):
    controls=[row['cases'][index]['cold_ttft_seconds'] for row in profiles if row['profile']!='local']
    gains.append(dict(case=case['case'],local_ttft=case['cold_ttft_seconds'],gpfs_controls=controls,
                      observed_ttft_reduction_percent=[100*(control-case['cold_ttft_seconds'])/control for control in controls]))
lut_compare=[dict(case=case['case'],lut_ttft=case['cold_ttft_seconds'],local_before=local['cases'][i]['cold_ttft_seconds'],
                  local_after=repeat['cases'][i]['cold_ttft_seconds'],lut_vs_after_change_percent=100*(case['cold_ttft_seconds']/repeat['cases'][i]['cold_ttft_seconds']-1))
             for i,case in enumerate(decoder['decoder_natural_cases'])]
summary=dict(schema='aster-xolotl-engram-comparison-r6',unix=time.time(),directory_comparison=profiles,local_storage_observed_gain=gains,
    decoder_cases=decoder['decoder_natural_cases'],decoder_long_summary=decoder['decoder_long_summary'],repeated_original_local_cases=repeat['cases'],
    decoder_comparison=lut_compare,decoder_api_gain_proven=False,decoder_adopted=False,decoder_control_binary_reproduced=True,
    exact_fp8_scaled_cases_per_mode_run=262144,exact_fp8_conversion_cases_per_mode_run=1024,cpu_modes=cpu['runs'],
    short_concurrency_probe=probe['result'],model_requests=57+19+7+len(probe['result']['tests']),
    actual1m_input_tested_this_round=False,original_v56_full1m_concurrency_gate_still_failed=True,
    formal_service_changed=False,maximum_authorized_h200=2,single_request300_met=False,goal_complete=False)
data(summary,'round-summary.json')
table=['| Profile | 8,244-token cold TTFT | 25,671-token cold TTFT | Short-input1024-output API tokens/s |','|---|---:|---:|---:|']
for row in profiles:table.append(f"| {row['profile']} | {row['cases'][0]['cold_ttft_seconds']:.6f}s | {row['cases'][1]['cold_ttft_seconds']:.6f}s | {row['long_summary']['api_wall_completion_tok_s']:.6f} |")
table.append(f"| lut_local | {decoder['decoder_natural_cases'][0]['cold_ttft_seconds']:.6f}s | {decoder['decoder_natural_cases'][1]['cold_ttft_seconds']:.6f}s | {decoder['decoder_long_summary']['api_wall_completion_tok_s']:.6f} |")
table.append(f"| original local repeat | {repeat['cases'][0]['cold_ttft_seconds']:.6f}s | {repeat['cases'][1]['cold_ttft_seconds']:.6f}s | not measured in this seven-request repeat |")
outcome='''# Natural-prefill Engram path comparison and rejected FP8 decoder (R6)

The same physical GPU355/native34f/API f062/S512/memo1 profile runs fresh
processes in GPFS/local/GPFS order. Only the Engram table directory changes;
all profiles explicitly use2M raw row-cache entries. The203GB original/local
full-SHA table proof is retained in R5. Original source page caches are not
evicted. This measures the complete storage path and its cache behavior,
not an isolated physical-disk or guaranteed cold-OS-cache effect.

'''+ '\n'.join(table)+'''

Each directory profile passes seven exact natural-input/KV-restore requests
plus twelve native-tool/1024-output requests,57 total. Inputs8244/25671 are
public technical documentation built from committed sources with exact
tokenizer counts and retained bytes/IDs. All three retrieval values and
restored complete output hashes match. Both GPFS controls remain slower
than the local path; continuous short-input decode has no material gain.

The opt-in FP8 lookup candidate covers all256 encodings and all256 scale
bytes under four rounding modes:262144 scaled and1024 conversion cases
per run. Control0,enabled1 andASAN/UBSAN enabled1 pass; invalid mode2 is
refused. Exact original and default0 Engram objects reproduce byte-for-byte.
The native34f control executable is reproduced before replacing only the
Engram archive member. The candidate passes19 real model requests with
unchanged tool, retrieval and complete1024-token outputs. Its apparent
improvement versus the first local block disappears against the repeated
original local control. No model/API gain is established and it is not adopted.

Two startup-budget refusals occurred before any model child existed.
Their original receipts/controllers are retained. Exact staged inputs,
private credential hash, absent child/result/log files, idle GPU and the
unchanged resource expiry were verified before recovering the same roots.
The second finite seven-request client has420s process/480s lease bounds;
its retrieval and output-identity checks are unchanged. No GPU was added.

The bounded short-input concurrency diagnostic retains all complete rows,
same/mixed-prompt pair walls, exact reference results and post-serial checks.
Its phase is '''+probe['result']['phase']+''' with '''+str(len(probe['result']['tests']))+''' recorded requests.
Read its failed/passed scope in round-summary.json; it cannot qualify or
replace the still-failed V56 actual1M concurrency suite.

## Commands and counts

- run_xolotl_engram_comparison_v71.py: first two19-request profiles pass;
  final bootstrap refuses its obsolete1800s startup floor before launching.
- recover_xolotl_unstarted_gpfs_b_v71r1.py and finish_xolotl_engram_and_lut_v78.py:
  exact final19-request directory profile, controlled native link and19 decoder requests pass.
- prepare_xolotl_engram_fp8_lut_v72.py, run_xolotl_engram_fp8_cpu_v72.py and
  build_xolotl_engram_fp8_cpu_v73.py: exhaustive CPU checks and exact object reproduction.
- repeat_xolotl_local_control_v82.py: original720s startup guard refusal retained;
  recover_xolotl_unstarted_local_repeat_v83.py and finish_xolotl_local_repeat_v84.py:
  seven unchanged original local natural-input controls pass.
- manage_xolotl_short_concurrency_v85.py launch/observe: finite original-native
  diagnostic; exact terminal result retained without changing its identity gate.
- collect_xolotl_engram_numeric_v71.py: bounded numeric source-linked model
  diagnostics and read-only formal observer proof. No inference on formal GPU.

## Explicitly not claimed

- No300/500/1000 token/s single-request qualification or goal completion.
- No actual1M input in this round, natural-language1M capacity proof,
  cold1M concurrency, long soak or general P95 guarantee.
- No FP8 lookup API speed gain/adoption; no full1M concurrent-identity fix
  or established root cause from a short-input diagnostic alone.
- No promotion to the formalGPU5539 service, no fresh WorkBuddy GUI or
  cold-first-turn acceptance and no Rust-native serving-runtime claim.
- No new Rust workspace CI/fmt claim. Only external candidate sources,
  public fixtures, operators and bounded evidence are retained; no model
  weights, binaries, compiled objects, credentials or private client state.
'''
put(outcome.encode(),'OUTCOME.md')
data(dict(schema='aster-xolotl-engram-comparison-r6-manifest',unix=time.time(),files=files),'manifest.json')
existing={row['file'] for row in old_rows}
for path in sorted(D.rglob('*')):
    if path.is_file():
        name=path.relative_to(E).as_posix()
        if name in existing:raise RuntimeError('immutable evidence collision')
        raw=path.read_bytes();manifest['files'].append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
if manifest['files'][:len(old_rows)]!=old_rows:raise RuntimeError('historical row order changed')
manifest.update(latest_engram_prefill_revision='ttft-1m-20261025/r6',goal_complete=False)
newline='\r\n' if b'\r\n' in original else '\n';manifest_path.write_bytes((json.dumps(manifest,indent=2)+'\n').replace('\n',newline).encode())
p=W/'docs/source-manifest.json';source=json.loads(p.read_text());entry=source['original_xolotl_h200_20261024'];relative=D.relative_to(W).as_posix()
entry['latest_engram_storage_candidate'].update(evidence=relative,model_api_gain_proven=True,model_api_gain_scope='One finite same-GPU fresh-process GPFS/local/GPFS block; actual8244/25671 natural technical tokens; no shared OS-cache eviction',
    actual1m_input_tested=False,natural_and_tool_long_requests=57,observed_gains=gains,production_promoted=False)
entry['latest_engram_fp8_candidate']=dict(evidence=relative,native_sha256=decoder['candidate_binary']['sha256'],exact_cpu_scaled_cases=262144,
    original_and_default_object_reproduced=True,native_control_reproduced=True,real_model_requests=19,gpu_correctness_qualified=True,
    model_api_gain_proven=False,repeat_control_rejected_speed_claim=True,production_promoted=False)
entry['latest_short_concurrency_probe']=dict(evidence=relative,phase=probe['result']['phase'],requests=len(probe['result']['tests']),
    concurrent_reference_matched=probe['result'].get('concurrent_reference_matched'),serial_post_matched=probe['result'].get('serial_post_matched'),full1m_qualified=False,cause_proven=False)
roles=entry['current_serving_optimization_roles'];roles.update(unix=time.time(),goal_met=False,
    next_action='Keep formal c016 unchanged. Rejected LUT has no isolated API gain; profile GPU/host execution and investigate exact concurrent-path outputs. New actual1M natural-input and full-window local-table tests remain required; any resource replacement follows exact terminal predecessor proof and max2 GPUs.')
roles['optimization'].update(current_profile='native34f/S512/f062/memo1/local Engram; repeated original control',production_promoted=False,
    actual1m_input_tested_this_round=False,latest_evidence=relative,local_path_api_gain_observed=True,fp8_lut_adopted=False)
roles['publication']['evidence_revision']='ttft-1m-20261025/r6'
p.write_text(json.dumps(source,indent=2,ensure_ascii=False)+'\n',encoding='utf-8',newline='\n')
p=W/'.gitattributes';rule='/docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200/ttft-1m-20261025/r6/frozen-candidates/**/*.patch -text whitespace=-blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol\n'
text=p.read_text()
if rule.strip() in text:raise RuntimeError('R6 patch-data attribute already exists')
p.write_text(text.rstrip()+'\n'+rule,encoding='utf-8',newline='\n')
print(json.dumps(dict(phase='retained',revision='r6',files=sum(path.is_file() for path in D.rglob('*')),historical_entries_preserved=len(old_rows),credential_occurrences=0,
    actual1m_input_tested=False,decoder_adopted=False,short_concurrency_phase=probe['result']['phase'],single_request300_met=False)))
