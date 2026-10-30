"""Append completed actual1M/SIMD/CPU proofs, with canonical metadata and explicit failed gates."""
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import time

B=Path('D:/code/aster/target/h200-native')
W=Path('D:/code/aster/.wt-h200-native-integration')
E=W/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200'
D=E/'ttft-1m-20261025/r8'
def read(name):return json.loads((B/name).read_text(encoding='utf-8-sig'))
matrix=read('xolotl-phase-simd-v140r1-controller.json')
full=read('xolotl-code-fullwindow-suite-v119-latest.json')
cpu=read('xolotl-engram-simd-cpu-v137-latest.json')
literal=read('xolotl-literal-scan-cpu-v145r1-latest.json')
api=read('xolotl-api-literal-v148-latest.json')
promotion=read('xolotl-stable-recovery-v157-promotion.json')
if D.exists() or subprocess.check_output(['git','status','--porcelain'],cwd=W):raise RuntimeError('R8 requires a new immutable revision and a clean integration worktree')
if matrix['phase']!='comparison_completed' or len(matrix['profiles'])!=3 or any(p['basic']['phase']!='passed' or p['extended']['phase']!='passed' or p['model_requests']!=40 for p in matrix['profiles']):raise RuntimeError('exact120 same-GPU SIMD requests incomplete')
q=full['fullwindow/request-result.json']
if full['exact_client_alive'] or full['suite-result.json']['phase']!='failed' or len(q['tests'])!=20 or q['error']!='ValueError: full-window long-output identity differs' or not q['exact_restored_long_output_matched'] or len(q['levels'])!=1:raise RuntimeError('exact20 real1M calls and failed C2 gate must be retained')
if cpu['exact_cpu_process_alive'] or cpu['cpu-result.json']['phase']!='compiled' or not cpu['cpu-result.json']['default0_object_byte_equal'] or [x['result']['scaled_value_cases'] for x in cpu['cpu-result.json']['tests']]!=[786432]*3:raise RuntimeError('exact SIMD exhaustive proof incomplete')
l=literal['cpu-result.json']
if literal['exact_cpu_process_alive'] or l['phase']!='passed' or not l['all_A_B_A_ids_exact'] or l['unit_methods_passed']!=15 or l['unchanged_upstream_frontend_methods_passed']!=25:raise RuntimeError('exact canonical API A/B/A proof incomplete')
if api['exact_cpu_process_alive'] or api['cpu-preflight-result.json']['phase']!='passed' or api['cpu-preflight-result.json']['tests_completed']!=14:raise RuntimeError('both-mode real API overlay CPU gates incomplete')
if promotion['phase']!='promoted' or not promotion['backend_verified_by_unique_nonce_and_kv_reuse']:raise RuntimeError('fresh same-VIP C11 recovery must finish before current-role publication')
keys=[]
for p in B.glob('access-private-*/api-key.txt'):
    value=p.read_bytes().strip()
    if len(value)>=24 and value not in keys:keys.append(value)
if not keys:raise RuntimeError('actual credential negative controls missing')
mp=E/'manifest.json';original=mp.read_bytes();manifest=json.loads(original);old=list(manifest['files'])
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
patterns=['xolotl-code-service-v118-*.json','xolotl-code-extended-v118-*.json','xolotl-code-fullwindow-suite-v119-*.json',
 'xolotl-matched-profile-code1m-v127-controller.json','xolotl-historical-120-reconciliation-v130.json',
 'xolotl-actual1m-*.json','xolotl-full1m-api-encoding-v141-*.json','xolotl-prefill-phase-cpu-v134-*.json',
 'xolotl-engram-simd-cpu-v137-*.json','xolotl-native-phase-simd-v138-*.json','xolotl-formal-readonly-status-v138.json',
 'xolotl-phase-simd-v139-*.json','xolotl-phase-simd-extended-v139-*.json','xolotl-phase-simd-v140-controller.json',
 'xolotl-phase-simd-v140r1-controller.json','xolotl-phase-simd-resume-v140r1-proof.json',
 'xolotl-literal-scan-cpu-v143-*.json','xolotl-literal-scan-cpu-v145-*.json','xolotl-literal-scan-cpu-v145r1-*.json',
 'xolotl-literal-scan-v145-prepared.json','xolotl-literal-scan-v145r1-prepared.json',
 'xolotl-canonical-code-fixture-v147-intent.json','xolotl-api-literal-v148-*.json','xolotl-h200-capacity-v154.json',
 'xolotl-formal-replacement-v155r1-repair.json','xolotl-stable-v97-early-stop-v156-reason.json',
 'xolotl-stable-recovery-v157-prepared.json','xolotl-stable-recovery-v157r1-*.json','xolotl-stable-recovery-v157-controller.json',
 'xolotl-stable-recovery-v157-stable-*.json','xolotl-stable-extended-v157-*.json','xolotl-stable-edge-suite-v157-*.json',
 'xolotl-stable-recovery-v157-promotion.json','xolotl-stable-service-v157-intent.json','xolotl-single1m-cont11-lease-pending.json',
 'xolotl-stable-recovery-v158-controller.json','xolotl-readiness-watch-v159-intent.json','xolotl-sealed-phase-simd-v139-*-intent.json','xolotl-sealed-native-variants-v138-intent.json',
 'xolotl-sealed-stable-recovery-v157-intent.json','xolotl-ncu-inventory-v161.json','xolotl-engram-host-budget-v162.json']
for name in sorted({p.name for pattern in patterns for p in B.glob(pattern)}):copy(name,'receipts/'+name)
operators=['launch_code_service_v118r1.frozen.py','manage_code_basic_v118.frozen.py','manage_code_extended_v118.frozen.py',
 'xolotl_code_fullwindow_long_worker_v119.frozen.py','xolotl_code_fullwindow_suite_worker_v119.frozen.py','manage_code_fullwindow_suite_v119.frozen.py',
 'run_matched_profile_and_code1m_v127.frozen.py','observe_actual1m_io_v131.py','prepare_prefill_phase_trace_v133.py',
 'prefill_phase_cpu_worker_v134.frozen.py','manage_prefill_phase_cpu_v134.py','prepare_engram_simd_v136.py',
 'engram_simd_cpu_worker_v137.frozen.py','manage_engram_simd_cpu_v137.py','manage_native_phase_simd_v138.py','native_phase_simd_link_worker_v138.frozen.py',
 'xolotl_sealed_bootstrap_v138.py','prepare_phase_simd_services_v139.py','launch_phase_simd_service_v139.frozen.py',
 'manage_phase_simd_basic_v139.frozen.py','manage_phase_simd_extended_v139.frozen.py','xolotl_sealed_bootstrap_v139.py',
 'run_phase_simd_matrix_v140.py','run_phase_simd_matrix_v140r1.frozen.py','prepare_phase_simd_resume_v140r1.py',
 'diagnose_full1m_encoding_v141.py','literal_scan_cpu_worker_v143.frozen.py','manage_literal_scan_cpu_v143.py',
 'prepare_literal_scan_replay_v145.py','literal_scan_cpu_worker_v145.frozen.py','manage_literal_scan_cpu_v145.py',
 'prepare_literal_scan_resume_v145r1.py','literal_scan_cpu_worker_v145r1.frozen.py','manage_literal_scan_cpu_v145r1.py',
 'collect_actual1m_simd_public_v146.py','stage_canonical_code_fixture_v147.py','prepare_api_literal_v148.py',
 'api_literal_canonical_probe_v148.frozen.py','api_literal_preflight_worker_v148.frozen.py','manage_api_literal_preflight_v148.py',
 'prepare_literal_services_v149.py','launch_literal_service_v149.frozen.py','manage_literal_basic_v149.frozen.py','manage_literal_extended_v149.frozen.py',
 'xolotl_sealed_bootstrap_v149.py','prepare_literal_fullwindow_suite_v150.py','manage_literal_fullwindow_suite_v150.py',
 'xolotl_literal_fullwindow_suite_worker_v150.frozen.py','run_literal_api_comparison_v151.frozen.py','prepare_formal_readonly_v152.py',
 'observe_formal_readonly_v152.py','checkpoint_h200_round_v153.py','observe_h200_capacity_v154.py',
 'prepare_formal_replacement_lease_v155.py','submit_xolotl_single1m_cont11_lease_v155.frozen.py','prepare_formal_replacement_lease_v155r1.py',
 'submit_xolotl_single1m_cont11_lease_v155r1.frozen.py','inspect_formal_early_stop_v156.py','prepare_xolotl_stable_recovery_v157.py',
 'xolotl_recovery_common_v157.py','xolotl_sealed_bootstrap_v157.py','launch_xolotl_stable_recovery_v157.frozen.py',
 'manage_xolotl_stable_recovery_v157.frozen.py','manage_xolotl_stable_extended_v157.frozen.py','manage_xolotl_stable_edge_suite_v157.frozen.py',
 'run_xolotl_stable_recovery_v157.frozen.py','prepare_stable_controller_v157r1.py','run_xolotl_stable_recovery_v157r1.frozen.py',
 'promote_xolotl_stable_recovery_v157.frozen.py','finalize_xolotl_stable_recovery_v157.frozen.py',
 'finalize_stable_recovery_driver_v158.frozen.py','prepare_readiness_watch_v159.py','launch_xolotl_readiness_watch_v159.frozen.py',
 'inspect_ncu_help_v161.py','observe_engram_host_budget_v162.py','checkpoint_formal_recovery_v163.py','verify_h200_r8_publication_v160.py',
 'xolotl_trial_common.py','xolotl_serving_common.py','retain_xolotl_actual1m_simd_literal_r8.py']
for name in operators:
    if not (B/name).is_file():raise RuntimeError('closed R8 source list missing: '+name)
    copy(name,'operators/'+name)
for folder in ['prefill-phase-trace-v133r1','engram-fp8-simd-v136','literal-scan-v142','literal-scan-v144','api-adapter-v148']:
    for path in sorted((B/folder).iterdir()):
        if path.is_file() and (path.suffix in {'.py','.hpp','.cpp','.patch','.json','.in'}):
            copy(path.relative_to(B).as_posix(),'frozen-candidates/'+path.relative_to(B).as_posix())
collection=read('xolotl-actual1m-simd-public-v146.json')
for row in collection['files']:
    p=B/'actual1m-simd-public-v146'/row['file'];raw=p.read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('closed remote diagnostic differs')
    put(raw,'public-diagnostics/'+row['file'],'actual1m-simd-public-v146/'+row['file'])
copy('xolotl-actual1m-simd-public-v146.json','public-diagnostics/collection-manifest.json')
metadata=literal['fixture-metadata-canonical.json'];data(metadata,'canonical-fixture/fixture-metadata.json')
data(dict(public_body='../../r7/code-fixture/public-fixture.txt.gz',public_body_sha256=metadata['prompt_sha256'],
    body_changed=False,protected_api_id_sha256=metadata['prompt_id_sha256'],native_runtime_id_digest_captured=False),
    'canonical-fixture/body-reference.json')
control=[p['extended']['long_summary']['api_wall_completion_tok_s'] for p in matrix['profiles'] if p['profile']!='simd']
simd=matrix['profiles'][1]['extended']['long_summary']['api_wall_completion_tok_s']
summary=dict(schema='aster-actual1m-simd-literal-r8',unix=time.time(),actual1m_input_tokens=1047544,actual_output_tokens=1024,
    fullwindow_suite_requests=20,actual1m_requests=18,remainder_passed=True,cold=q['tests'][1],restored=q['tests'][3],c1=q['levels'][0],
    c2_structurally_completed=8,c2_full_output_identity_gate_passed=False,simd_model_requests=120,
    simd_profiles=matrix['profiles'],simd_observed_api_wall_gain_percent=[100*(simd/value-1) for value in control],
    simd_actual1m_input_tested=False,simd_exhaustive_values_per_mode=786432,literal_cpu_A_B_A=l['runs'],
    literal_all_ids_exact=True,canonical_api_id_sha256=metadata['prompt_id_sha256'],
    old_direct_template_id_sha256=metadata['previous_direct_template_prompt_id_sha256'],
    canonical_id_difference_count=l['direct_and_protected_token_differences'],native_runtime_id_digest_captured=False,
    new_api_cpu_suites=14,formal_recovery=promotion,experimental_candidates_promoted=False,
    actual1m_literal_gpu_comparison_in_progress=True,goal_complete=False,single_request300_met=False)
data(summary,'round-summary.json')
outcome='''# Actual unique-code1M, SIMD and literal preprocessing (R8)

The unique public-code request uses1047544 input tokens,1024 output and8
safety. Its first cold request completes in2050.044s with2012.482s TTFT;
native prefill is2004.21s (522.7 input tokens/s). Cold decode is27.24token/s.
The restored response matches the complete cold output hash. Eight serial
cached requests emit8192 tokens in351.105s:23.3321 API-wall token/s,28.4002
stream-interval token/s and7.8553s TTFT P50. All eight concurrent2 calls
finish1024 tokens and retrieve all three codes, but their full output hashes
differ. The original C2 identity gate fails and remains unchanged. This is
not a qualified concurrent-throughput result or a controlled comparison to
the earlier repeated-filler prompt.

Native numeric phases locate1208.740s in dense GPU work and399.158s in
Engram waits across41080 attention spans; summed nested counters may overlap.
Cached native prefill is0.02–0.03s while API TTFT is about7.9s. Repeated
bounded process samples show actual GPFS row reads blocked in pread64. These
observations guide both GPU work and frontend/I/O optimizations.

The AVX2 Engram decoder preserves all786432 FP8/scale/rounding/alignment
cases in each of control, enabled and ASAN/UBSAN modes. The default-off
object and full parent native binary reproduce byte-for-byte. Only the
Engram member changes for the candidate; dense CUDA code remains identical.
Its37.57x CPU conversion microbenchmark is not a model speed result.

The same GPU099e runs trace-control / SIMD / trace-control, each with28
basic and12 native-tool/matched1024 calls. All120 pass unchanged gates.
API-wall long-output rates are75.742894 /76.311310 /75.503822 token/s:
only0.75–1.07% observed improvement. The35700 cold TTFTs are37.944368 /
37.938605 /38.581855s. Real1M SIMD input has not been tested and this
candidate has not replaced the formal service. The observer's100-second
timeout is retained; exact remote client completion was observed and the
same matrix resumed without replaying completed requests.

CPU replay finds two expensive literal passes: marking3.19s and unmarking
4.48s. The bounded default-off scanner matches declared tokenizer literals
exactly; unsafe priority overlaps use the original code. Contiguous marker
code points are compressed into exact regex ranges with no gaps included.
The real Service.encode_prompt A/B/A times are9.153778 /1.647161 /9.126680s,
an approximately82% CPU encoding reduction with identical complete ID arrays.
Fifteen hermetic helper methods and25 unchanged generic frontend methods pass.
The first generic-test attempt after DS41 globally patched Service is retained
as a harness failure; the successful revision isolates generic tests in a
fresh child before importing DS41. The actual overlay then passes14 CPU
suites across both startup flags, including the complete real1M encoder.
This is CPU evidence; GPU/API TTFT and decode gains remain pending.

The earlier public fixture metadata used direct template tokenization.
Actual API literal protection changes six ID values, preserving1047544 count,
body bytes, and record positions36/523698/1047424. The canonical ID digest is
b442a593a4066e3e99b663b670db4d7d624c471f1c5f6ee906e315c99d1969c4;
the old direct-template digest ebe492 is retained as history. GPU V119
verifies the submitted body, actual count and retrieval, but did not capture
the native runtime input-ID digest. The corrected canonical metadata is
append-only; no old evidence or input body is rewritten. A same-native,
same-GPU real1M baseline/literal API campaign is running separately.
The C11 formal-service recovery overlaps its control cold-prefill run;
any cold-prefill difference needs a quiet repeat before attribution.

A read-only H200 snapshot during actual1M prefill records98% busy,
1980MHz SM/3201MHz memory matching reported maximum clocks,700W power limit,
48C GPU temperature and no active clock-throttle flags. These readings are
not Tensor Core efficiency, achieved bandwidth or a hardware speed ceiling.
The installed Nsight Compute2024.3 CLI is inventoried and the driver reports
RmProfilingAdminOnly0. No hardware counter collection has been executed yet.

C9 is unexpectedly Stopped before its recorded lease; cause and actor are
unproven. Exact terminal job/replica evidence and failed Windows health are
retained. C11 replaces only that terminal capacity while C10 continues,
preserving maximum2 H200. C11 keeps the qualified nativefa/S128/API f062
profile and same VIP/key, passes fresh28+12+strict132097 and Windows unique
backend-nonce cache reuse. No experimental candidate is promoted. Two
prelaunch operator-reference errors are retained; neither created an extra
allocation nor signalled a model process.

## Commands and scope

- manage_code_fullwindow_suite_v119.frozen.py: strict132097 plus20
  fullwindow-suite calls, including18 actual1M calls; cold/restore/c1 pass,
  complete-output C2 identity fails.
- manage_engram_simd_cpu_v137.py / manage_native_phase_simd_v138.py:
 786432 cases per mode, sanitizer proof and exact parent reproduction.
- run_phase_simd_matrix_v140r1.frozen.py:120 completed model calls across
  three same-GPU profiles, no formal service change.
- manage_literal_scan_cpu_v145r1.py:15 helper +25 isolated upstream methods
  and real protected API ID-exact A/B/A;14 API-overlay CPU suites via V148.
- run_literal_api_comparison_v151.frozen.py: pending real1M GPU/API comparison,
  unchanged20-call/cold/restore/c1/c2 gates and a finite7200s campaign budget.
- run_xolotl_stable_recovery_v157r1.frozen.py and promotion/finalization:
  exact C11 resource, fresh-instance gates and same-VIP Windows verification.
- collect_actual1m_simd_public_v146.py:55 closed bounded public diagnostics,
  actual private-key negative controls. Active control_b engine log excluded.

No300/1000 single-request achievement, goal completion, full DFlash/DeepInfer
integration, fresh WorkBuddy GUI acceptance, actual1M formal C11 execution,
new Rust workspace CI/fmt result, raw profiler report, model weight, binary,
credential or private client state is claimed or published here.
'''
put(outcome.encode(),'OUTCOME.md')
data(dict(schema='aster-actual1m-simd-literal-r8-manifest',unix=time.time(),files=files),'manifest.json')
known={r['file'] for r in old}
for path in sorted(D.rglob('*')):
    if path.is_file():
        name=path.relative_to(E).as_posix();raw=path.read_bytes()
        if name in known:raise RuntimeError('immutable evidence path collision')
        manifest['files'].append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
if manifest['files'][:len(old)]!=old:raise RuntimeError('historical manifest order changed')
manifest.update(latest_actual1m_simd_literal_revision='ttft-1m-20261025/r8',goal_complete=False)
newline='\r\n' if b'\r\n' in original else '\n'
mp.write_bytes((json.dumps(manifest,indent=2)+'\n').replace('\n',newline).encode())
p=W/'docs/source-manifest.json';source=json.loads(p.read_text());entry=source['original_xolotl_h200_20261024'];relative=D.relative_to(W).as_posix()
entry['latest_unique_code_actual1m_fixture']=dict(evidence=relative,actual_input_tokens=1047544,max_output_tokens=1024,
    source_revision=metadata['source_revision'],corpus_repeated=False,canonical_api_cpu_qualified=True,
    canonical_api_prompt_id_sha256=metadata['prompt_id_sha256'],gpu_actual_count_verified=True,native_runtime_id_digest_captured=False,
    full_cold_restore_and_c1_passed=True,complete_c2_output_identity_passed=False)
entry['latest_engram_simd']=dict(evidence=relative,model_requests=120,all_standard_requests_passed=True,
    observed_long_output_api_tok_s=[p['extended']['long_summary']['api_wall_completion_tok_s'] for p in matrix['profiles']],
    real1m_gpu_input_tested=False,production_promoted=False)
entry['latest_exact_literal_frontend']=dict(evidence=relative,real_api_cpu_A_B_A_seconds=[r['actual_service_encode_seconds'] for r in l['runs']],
    all_token_ids_exact=True,new_helper_unit_methods=15,upstream_frontend_methods=25,overlay_cpu_suites=14,
    gpu_api_improvement_qualified=False,production_promoted=False)
target=read('xolotl-stable-service-v157-intent.json');roles=entry['current_serving_optimization_roles']
old_public=roles['serving'];formal=dict(old_public)
formal.update(allocation_uid=target['allocation_uid'],allocation_job=target['allocation_job'],replica_uid=target['replica_uid'],gpu_uuid=target['gpu_uuid'],
    lease_ends_unix=target['plan']['lease_ends_unix'],binary_sha256=target['plan']['binary_sha256'],
    availability_evidence=relative+'/receipts/xolotl-stable-recovery-v157-promotion.json',
    fresh_windows_backend_verified=True,workbuddy_bundled_cli_e2e_passed=False,workbuddy_gui_end_to_end_passed=False,
    instance_full_1m_execution_tested=False,actual_prompt_tokens_qualified=132097)
roles.update(unix=time.time(),serving=formal,active_service=dict(formal),goal_met=False,maximum_allocated_h200=2,
    next_action='Observe exact same-native literal API real1M campaign on C10; preserve failed C2 gate, current C11 availability and bounded max2 roles; target dense GPU/kernel/copy/sync counters after finite campaigns.')
roles['optimization'].update(allocation_uid='7455d989-8255-4f7c-ae8f-3cc0aa5928e4',gpu_uuid='GPU-099e8da4-8df0-c431-9e67-eb721e59beee',
    latest_evidence=relative,current_profile='Same-native99f3/S512/GPFS baseline vs exact literal API; real1M campaign in progress',
    actual1m_input_completed_this_round=True,full1m_candidate_qualified=False,production_promoted=False,
    concurrent_identity_fixed=False,ordinary_decode_gain_proven=False)
roles['publication']['evidence_revision']='ttft-1m-20261025/r8'
p.write_text(json.dumps(source,indent=2,ensure_ascii=False)+'\n',encoding='utf-8',newline='\n')
p=W/'.gitattributes';text=p.read_text();rule='/docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200/ttft-1m-20261025/r8/frozen-candidates/**/*.patch -text whitespace=-blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol\n'
p.write_text(text.rstrip()+'\n'+rule,encoding='utf-8',newline='\n')
print(json.dumps(dict(phase='retained',revision='r8',files=sum(p.is_file() for p in D.rglob('*')),historical_entries_preserved=len(old),
    same_gpu_simd_requests=120,fullwindow_suite_requests=20,actual1m_requests=18,actual1m_c2_passed=False,experimental_candidates_promoted=False,goal_complete=False)))
