"""Append actual38 GPU Top-K gates, retained sampling failures, and verified C13 recovery."""
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import time

B=Path(__file__).resolve().parent;W=Path('D:/code/aster/.wt-h200-native-integration')
E=W/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200'
D=E/'ttft-1m-20261025/r10'
def read(name):return json.loads((B/name).read_text(encoding='utf-8-sig'))
if D.exists() or subprocess.check_output(['git','status','--porcelain'],cwd=W):raise RuntimeError('new R10 requires a clean integration worktree')
operator=read('xolotl-topk-harness-v203-gpu-latest.json');o=operator['gpu-result.json']
profile=read('xolotl-ncu-actual1m-v172r2-latest.json');p=profile['result.json']
smoke=read('xolotl-ncu-smoke-recovery-v207.json')['result'];geometry=read('xolotl-exact-topk-geometry-v223r1.json')['result']['geometry']
promotion=read('xolotl-stable-recovery-v217-promotion.json');target=read('xolotl-stable-service-v217-intent.json')
basic=read('xolotl-stable-recovery-v217-stable-qualification-latest.json');extended=read('xolotl-stable-extended-v217-stable-latest.json');edge=read('xolotl-stable-edge-suite-v217-latest.json')
latest=read('xolotl-current-role-progress-v219-latest-path.json');health=read(latest['file'])
pipeline=read('xolotl-topk-native-model-v213r1-controller.json')
if operator['alive'] or o['phase']!='passed' or len(o['cases'])!=38 or not o['engine_reaped'] or not o['gpu_idle_after'] or any(not row['all_IDs_equal'] or not row['graph_changed_input_exact'] for row in o['cases']):raise RuntimeError('exact38 ordered-ID/CUDA-graph GPU gates required')
if profile['exact_worker_alive'] or p['phase']!='failed' or p['hardware_counters_collected'] or not p['engine_reaped'] or not p['native_target_gone'] or len(p['requests'])!=3:raise RuntimeError('exact failed/reaped real1M model profiler must remain visible')
ids=[[t['id'] for t in row['tokens']] for row in p['requests']]
if not all(row['prompt_tokens']==1047544 and row['completion_tokens']==128 for row in p['requests']) or any(len(row)!=128 or row!=ids[0] for row in ids):raise RuntimeError('three completed actual1M diagnostic controls differ')
if smoke['phase']!='passed' or len(smoke['numeric_metrics'])!=8 or not smoke['hardware_counters_collected'] or smoke['model_hardware_counters_collected'] or geometry['index_topk']!=512 or geometry['context_length']!=1048576:raise RuntimeError('exact tiny-counter/declared model geometry proof differs')
if promotion['phase']!='promoted' or not promotion['backend_verified_by_unique_nonce_and_kv_reuse'] or promotion['new_allocation_uid']!='e808d0b3-65c6-4a48-b121-220ff6fc5eb2' or basic['result']['phase']!='passed' or len(basic['result']['tests'])!=28 or extended['result']['phase']!='passed' or len(extended['result']['tests'])!=12 or edge['suite-result.json']['phase']!='passed' or edge['remainder/request-result.json']['usage']['prompt_tokens']!=132097:raise RuntimeError('all exact C13 stable40/132097/Windows recovery gates required')
if time.time()-health['unix']>150 or health['formal']['uid']!=target['allocation_uid'] or health['formal']['phase']!='Running' or health['Windows_health'].get('status')!=200 or health['optimization']['uid']!='b37b7201-2c9c-45a2-ae1f-e522319324ca' or health['optimization']['phase']!='Running':raise RuntimeError('fresh exact two-role/Windows health proof required')
if pipeline['phase']!='actual1m_fullwindow' or pipeline['completed_profiles'] or pipeline['candidate_promoted'] or pipeline['model_speed_claimed']:raise RuntimeError('full model comparison must remain explicitly pending in this snapshot')
keys=list({p.read_bytes().strip() for p in B.glob('access-private-*/api-key.txt') if len(p.read_bytes().strip())>=24})
if len(keys)<2:raise RuntimeError('both actual credential negative controls required')
mp=E/'manifest.json';original=mp.read_bytes();manifest=json.loads(original);old=list(manifest['files'])
if len(old)!=2645:raise RuntimeError('exact R9 manifest count differs')
for row in old:
    raw=(E/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('historical evidence bytes differ: '+row['file'])
D.mkdir(parents=True);files=[]
def put(raw,name,source=None):
    if len(raw)>2*1024**2 or any(key in raw for key in keys):raise RuntimeError('bounded actual-credential publication gate refused: '+name)
    if name.endswith('.py'):ast.parse(raw,filename=name)
    if name.endswith('.json'):json.loads(raw)
    path=D/name;path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as out:out.write(raw)
    files.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),source=source))
def copy(name,dest=None):put((B/name).read_bytes(),dest or 'receipts/'+name,name)
def data(value,name):put((json.dumps(value,indent=2,ensure_ascii=False)+'\n').encode(),name)
receipts=[
    'xolotl-actual-topk-build-v187.json','xolotl-topk-fallback-references-v189.json','xolotl-cccl-release-v190.json',
    'xolotl-cccl-headers-v191-intent.json','xolotl-cccl-headers-v191-latest.json','xolotl-topk-compile-v193-intent.json','xolotl-topk-compile-v193-latest.json',
    'xolotl-topk-harness-v195-intent.json','xolotl-topk-harness-v195-compile-latest.json',
    'xolotl-topk-harness-v195r1-intent.json','xolotl-topk-harness-v195r1-compile-latest.json',
    'xolotl-profiler-prefix-failure-v196.json','xolotl-protocol-r2-v198-prepared.json',
    'xolotl-ncu-actual1m-v172r2-plan.json','xolotl-ncu-actual1m-v172r2-intent.json',
    'xolotl-fullwindow-hardware-v173r2-controller.json','xolotl-topk-native-bridge-v202-intent.json','xolotl-topk-native-bridge-v202-latest.json',
    'xolotl-topk-harness-prerequisite-v203-prepared.json','xolotl-topk-harness-v203-intent.json','xolotl-topk-harness-v203-compile-latest.json','xolotl-topk-harness-v203-gpu-intent.json','xolotl-topk-harness-v203-gpu-latest.json',
    'xolotl-ncu-smoke-v205-intent.json','xolotl-ncu-smoke-recovery-v207.json',
    'xolotl-next-optimization-lease-v208-prepared.json','xolotl-optimization-c10-retirement-v209.json','xolotl-c12-observer-v210-activation.json',
    'xolotl-topk-native-v212-source-manifest.json','xolotl-topk-native-v212-control_a-intent.json','xolotl-topk-native-v212-control_a-qualification-latest.json',
    'xolotl-topk-native-extended-v212-control_a-latest.json','xolotl-topk-native-model-v213-controller.json','xolotl-topk-native-model-v213r1-controller.json',
    'xolotl-topk-fullwindow-v214-prepared.json','xolotl-post-recovery-v219-prepared.json',
    'xolotl-formal-c13-v216-prepared.json','xolotl-stable-recovery-v217-prepared.json','xolotl-stable-recovery-v217-controller.json',
    'xolotl-stable-recovery-v217-stable-intent.json','xolotl-stable-recovery-v217-stable-qualification-latest.json',
    'xolotl-stable-extended-v217-stable-latest.json','xolotl-stable-edge-suite-v217-latest.json','xolotl-stable-recovery-v217-promotion.json',
    'xolotl-readiness-watch-v220-intent.json','xolotl-topk-geometry-v223-failure.json','xolotl-exact-topk-geometry-v223r1.json']
for name in receipts:copy(name)
copy(latest['file'],'receipts/fresh-current-role-observation.json')
for name in ['xolotl-ncu-actual1m-v172r1-latest.json','xolotl-ncu-actual1m-v172r2-latest.json']:
    row=read(name);filtered={key:row[key] for key in ['unix','exact_worker_alive','result.json','worker.log','engine.stderr.log'] if key in row}
    filtered['retained_profiler_error_lines']=[line for line in row.get('private-ncu.log','').splitlines() if line.startswith('==ERROR==')]
    filtered['raw_profiler_report_or_CSV_published']=False;data(filtered,'receipts/'+name)
row=read('xolotl-ncu-smoke-v205-latest.json')
data({key:value for key,value in row.items() if key!='private-ncu.csv'},'receipts/xolotl-ncu-smoke-v205-latest-sanitized.json')
discovery=read('xolotl-nsys-metrics-cli-v226.json');help_bytes=discovery['profile_help']['stdout'].encode()
data(dict(binary=discovery['binary'],version=discovery['version']['stdout'].strip(),
    profiling_started=False,profile_help_bytes=len(help_bytes),profile_help_sha256=hashlib.sha256(help_bytes).hexdigest(),
    cuda_visible_metric_selector_declared='cuda-visible' in discovery['gpu_metric_devices']['stdout'],
    plural_metric_devices_option_declared='--gpu-metrics-devices=' in discovery['profile_help']['stdout']),
    'receipts/installed-nsys-metrics-support.json')
stop=read('xolotl-formal-c11-stop-v215.json');policy=read('xolotl-rjob-lifecycle-policy-v218.json')
data(dict(rjob={key:stop['rjob'][key] for key in ['status']},replica_status=stop['replica']['status'],
    old_allocation_uid=stop['rjob']['metadata']['uid'],last_worker_health=stop['formal_worker'],last_watch=stop['watch'],
    recorded_lease_ends_unix=stop['recorded_lease_ends_unix'],terminal_before_lease=stop['terminal_before_recorded_lease'],
    cause_identified=False,actor_identified=False,declared_lifecycle={key:policy[key] for key in ['job_spec_non_tasks','task_fields','declared_labels','policy_change_executed']}),
    'receipts/formal-C11-stop-and-declared-policy.json')
operators=[
    'inspect_actual_topk_build_v187.py','fetch_cccl_topk_source_v188.py','fetch_cccl_local_v189.py','fetch_cccl_local_v189r1.py','stage_cccl_headers_v191.py',
    'aster_topk_order_v192.hpp','aster_topk35_api_v192.hpp','aster_topk_cccl35_v192.cu','test_topk_order_v192.cpp','compile_topk_cccl35_v193.py',
    'bench_topk_cccl35_v194.cpp','manage_topk_harness_v195.py','manage_topk_harness_v195r1.py',
    'aster_native_resume_v197.py','test_native_resume_v197.py','prepare_native_resume_and_topk_r2_v198.py',
    'xolotl_ncu_actual1m_worker_v171r2.frozen.py','manage_ncu_actual1m_v172r2.py','xolotl_sealed_bootstrap_v172r2.py','run_fullwindow_then_hardware_profile_v173r2.frozen.py',
    'observe_h200_prefill_telemetry_v200.py','aster_topk_native_bridge_v201.cu','compile_topk_native_bridge_v202.py',
    'prepare_operator_after_failed_profile_v203.py','manage_topk_harness_v203.py','h200_ncu_smoke_v204.cu','manage_h200_ncu_smoke_v205.py',
    'aster_ncu_wide_metrics_v206.py','test_ncu_wide_metrics_v206.py','recover_ncu_smoke_and_memory_v207.py',
    'prepare_next_optimization_lease_v208.py','submit_xolotl_single1m_cont12_lease_v208.frozen.py','retire_idle_optimization_c10_v209.py','activate_c12_observation_v210.py',
    'observe_current_h200_roles_v211.py','prepare_topk_native_services_v212.py','launch_topk_native_service_v212.frozen.py','manage_topk_native_basic_v212.frozen.py','manage_topk_native_extended_v212.frozen.py','xolotl_sealed_bootstrap_v212.py',
    'run_topk_native_model_aba_v213.frozen.py','run_topk_native_model_aba_v213r1.frozen.py','prepare_topk_fullwindow_suite_v214.py','manage_topk_fullwindow_suite_v214.py',
    'inspect_formal_c11_stop_v215.py','prepare_formal_c13_v216.py','submit_xolotl_single1m_cont13_lease_v216.frozen.py','prepare_stable_c13_recovery_v217.py',
    'xolotl_recovery_common_v217.py','xolotl_sealed_bootstrap_v217.py','launch_xolotl_stable_recovery_v217.frozen.py','manage_xolotl_stable_recovery_v217.frozen.py','manage_xolotl_stable_extended_v217.frozen.py','manage_xolotl_stable_edge_suite_v217.frozen.py','run_xolotl_stable_recovery_v217.frozen.py','promote_xolotl_stable_recovery_v217.frozen.py','finalize_xolotl_stable_recovery_v217.frozen.py',
    'inspect_rjob_policy_v218.py','prepare_post_recovery_observers_v219.py','observe_current_h200_roles_v219.py','prepare_readiness_watch_v220.py','launch_xolotl_readiness_watch_v220.frozen.py',
    'inspect_exact_model_geometry_v223.py','inspect_exact_model_geometry_v223r1.py','prepare_metadata_geometry_release_v224.py','inspect_nsys_metrics_cli_v226.py','retain_topk_gpu_and_c13_r10_v227.py']
for name in operators:copy(name,'operators/'+name)
audit=read('xolotl-actual-topk-build-v187.json');topk=next(row for row in audit['source_files'] if row['path'].endswith('/top-k.cu'))
put(topk['body'].encode(),'frozen-candidate/top-k.original.cu','exact upstream source from audit187')
copy('xolotl-trial-source-e9d0826d71c5/llama-source/LICENSE','frozen-candidate/llama-LICENSE')
sampling=list(B.glob('xolotl-h200-prefill-telemetry-v200-*.json'))
if len(sampling)!=1:raise RuntimeError('exact three-sample telemetry receipt differs')
copy(sampling[0].name)
finite=[{key:row[key] for key in ['cols','rows','pattern','reference_host_sync_us','candidate_host_sync_us','candidate_graph_host_sync_us']} for row in o['cases'] if row['pattern']==0]
summary=dict(schema='aster-h200-topk-GPU-and-formal-recovery-r10',unix=time.time(),operator_GPU_cases=38,operator_all_ordered_IDs_and_graph_replays_equal=True,
    random_finite_operator_cases=finite,small4097_operator_regression_retained=True,native_bridge_min_cols=65536,native_bridge_k=512,native_bridge_default_mode=0,
    model_geometry=geometry,official_CCCL_commit='28101b07413743e1dcca8ca8af1cfc63e8cff2d1',official_CCCL_release='3.5.0',
    model_profiler_phase='failed',completed_actual1m_diagnostic_controls=3,diagnostic_output_tokens=128,model_hardware_counters_collected=False,
    tiny_kernel_hardware_counters_collected=True,tiny_counter_metrics=8,own_C10_memory_peak_bytes=236223201280,own_C10_oom_kill_count=1,profiler_failure_cause_proven=False,
    formal_C11_early_stop_retained=True,formal_stop_actor_or_cause_proven=False,formal_C13_restored=True,formal_native_SHA=target['plan']['binary_sha256'],formal_prefill_chunk_tokens=128,
    formal_basic28_and_extended12_passed=True,formal_actual_prompt_tokens=132097,formal_new_instance_actual1m_tested=False,fresh_WorkBuddy_GUI_tested=False,
    formal_8x1024=extended['result']['long_summary'],Windows_nonce_and_cache_reuse=promotion['windows_direct_generation'],same_VIP_and_key=True,
    current_model_comparison=dict(controller='213r1',phase=pipeline['phase'],profile=pipeline['profile'],basic28_and_extended12_passed=True,
        actual1m_1024_output_comparison_completed=False,model_performance_gain_qualified=False,formal_promoted=False),
    prior_C2_full_output_identity_failure_preserved=True,goal_complete=False,single_request300_met=False)
data(summary,'round-summary.json')
outcome='''# H200 Top-K GPU comparison and stable-service recovery (R10)

The actual CUDA12.6.20/CUB2.5 engine links a full-argsort Top-K fallback.
The isolated official CCCL3.5.0 packed-key selection prototype passes all38
original-reference ordered-index cases, including ties, signed zero, NaN/Inf,
changed inputs and CUDA Graph replays. Both release and ASAN/UBSAN modes
pass14,680,074 integer-order contracts. No model forward is performed by
the operator harness.

For random finite1M-column inputs, synchronized operator host-wall times
change from100.484766us to61.133203us for one row,7255.715430us to301.214648us
for six rows, and7700.772266us to811.849219us for17 rows (candidate graph).
The six-row observation is approximately24.1x. These are operator timings,
not model/API token rates. All4097-column cases regress; the private native
bridge leaves the original path below65536 columns and for K other than512.
The bridge defaults off and refuses unknown mode values before main/model
loading. The original native99f3 control reproduces byte-for-byte; the separate
candidate2bc83 is compiled without replacing the stable native or archives.
Exact model metadata confirms Top-K512,40 layers and eight index-source layers.

The real1M profiler's first revision incorrectly required RESUME0, although
the native protocol emits the marker only for positive reuse. The cold128-ID
completion and failure are retained. Eighteen numeric/query/native-protocol
parser methods pass before R2. R2 then completes three1047544-input/128-output
controls with identical IDs. The fourth GEN reaches its decode marker, but
Nsight Compute aborts its first mul_mat_vec_q capture and the target exits9.
No actual-model hardware counters are collected. Native/worker cleanup is
verified. A later own-cgroup observation shows a220GiB peak and one OOM kill;
the exact killed process and causal allocation are not established.

A separate bounded tiny CUDA kernel correctly writes65536 words and obtains
all eight hardware metrics. The old parser assumes long-format CSV, while
installed NCU raw output is wide with a units row. Its failed receipt remains.
Seven hermetic wide-parser methods pass, and the same existing CSV is reparsed
successfully without another GPU run. Only eight whitelisted numeric metrics
are published. They characterize the tiny kernel, not model utilization.
Raw reports and CSV remain private. Nsight Systems2025.3.1 discovery supports
one visible H200 and cuda-visible metric sampling; no such sampling is started
in this revision, preserving the timed model campaign.

After all C10 workers finish and the optimization GPU is idle, C10 is stopped
and verified terminal before one replacement C12 is submitted. C12 obtains
the same physical H200099e, preserving the independent formal card and the
maximum two-GPU scope. The same native2bc83 binary is prepared for flag0/1/0
API A/B/A with the unchanged40 ordinary and20 fullwindow calls per profile.

At06:53:29UTC the independent formal C11 unexpectedly becomesStopped before
its recorded lease. Its worker health was200 approximately two seconds earlier.
The experimental controller stops subsequent work. Exact status and declared
lifecycle policy are retained: maxRunningDuration0s, autoDelete720h and
nonpreemptible. Neither actor nor root cause is established. C11 is terminal
before replacement C13 is requested; maximum allocated task GPUs remains two.

C13 recovers the same qualified nativefa019/S128/API f062 stable profile.
Fresh28 basic,12 native-tool/1024-output, and strict132097-input gates pass.
The existing VIP/key is switched and Windows nonce/cache-reuse verification
passes, with a47ms first token for the short nonce call. The stable8x1024
API-wall result is72.941584 token/s. Fresh C13 actual1M execution and WorkBuddy
GUI acceptance are not claimed. Read-only readiness observation is restored.

The prepared213r1 controller resumes the existing C12 control_a service and
completed basic-client handle after formal recovery, passes the12 extended
gates, and starts the complete actual1M suite. Its comparison remains pending.
The prior C2 full-output identity failure remains visible; no Top-K model gain,
experimental formal promotion,300/1000 achievement or goal completion is claimed.

## Scope and reproduction

- manage_topk_harness_v203.py: compiled actual original-reference operator
  harness; launch_gpu once then observe_gpu, all38 GPU gates passed.
- compile_topk_native_bridge_v202.py: separate default-off candidate, parent
  binary reproduction and seven invalid-mode-before-main refusals.
- manage_ncu_actual1m_v172r2.py observe: existing failed/reaped diagnostic.
- recover_ncu_smoke_and_memory_v207.py: seven wide-parser tests plus existing
  tiny-kernel CSV recovery; no additional GPU execution.
- run_topk_native_model_aba_v213r1.frozen.py: existing C12 full-window A/B/A;
  observe_current_h200_roles_v219.py verifies C13/C12 and Windows health.
- run_xolotl_stable_recovery_v217.frozen.py, promote/finalize217: completed
  unchanged stable40/132097 and Windows backend/cache verification.

No binary, weights, third-party header archive, raw profiler report/CSV,
credential, private client state or new Rust workspace CI/fmt result is published.
'''
put(outcome.encode(),'OUTCOME.md')
data(dict(schema='aster-h200-topk-r10-manifest',unix=time.time(),files=files),'manifest.json')
known={row['file'] for row in old}
for path in sorted(D.rglob('*')):
    if path.is_file():
        name=path.relative_to(E).as_posix();raw=path.read_bytes()
        if name in known:raise RuntimeError('immutable evidence collision')
        manifest['files'].append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
manifest.update(latest_topk_GPU_and_formal_recovery_revision='ttft-1m-20261025/r10',goal_complete=False)
newline='\r\n' if b'\r\n' in original else '\n';mp.write_bytes((json.dumps(manifest,indent=2)+'\n').replace('\n',newline).encode())
sp=W/'docs/source-manifest.json';source=json.loads(sp.read_text());entry=source['original_xolotl_h200_20261024'];relative=D.relative_to(W).as_posix()
entry['latest_exact_topk_GPU']=dict(evidence=relative,operator_cases=38,all_ordered_IDs_equal=True,CUDA_graph_changed_inputs_passed=True,
    CCCL_commit=summary['official_CCCL_commit'],native_candidate_sha256=pipeline['native_binary_sha256'],default_mode=0,min_columns=65536,k=512,
    operator_1M_rows6_reference_us=7255.71543,operator_1M_rows6_candidate_graph_us=301.214648,model_gain_qualified=False,production_promoted=False)
entry['latest_exact_hardware_profiler']=dict(evidence=relative,actual_model_phase='failed',actual_model_hardware_counters_collected=False,
    completed_actual1m_controls=3,output_tokens=128,tiny_kernel_metric_preflight_passed=True,tiny_kernel_metrics=8,raw_reports_published=False,failure_cause_proven=False)
roles=entry['current_serving_optimization_roles'];active=dict(roles['active_service']);active.update(allocation_uid=target['allocation_uid'],allocation_job=target['allocation_job'],replica_uid=target['replica_uid'],gpu_uuid=target['gpu_uuid'],lease_ends_unix=target['plan']['lease_ends_unix'],availability_evidence=relative+'/receipts/xolotl-stable-recovery-v217-promotion.json',actual_prompt_tokens_qualified=132097,full_1m_length_qualified=False,instance_full_1m_execution_tested=False,workbuddy_gui_end_to_end_passed=False,same_existing_key=True,fresh_windows_backend_verified=True)
roles.update(unix=time.time(),goal_met=False,active_service=active,serving=dict(active),next_action='Observe exact213r1/214 existing actual1M control_a; no duplicate input. Compare native2bc83 flag0/1/0 on C12; keep stable C13 and failed profiler/C2 gates visible. Do not equate24.1x operator timing with model/API speed.')
roles['optimization'].update(allocation_uid='b37b7201-2c9c-45a2-ae1f-e522319324ca',allocation_job='aster-xolotl-1mc12-00686222ad57-74805472',lease_ends_unix=health['optimization']['lease_ends_unix'],binary_sha256=pipeline['native_binary_sha256'],latest_evidence=relative,current_profile='Native Top-K flag0/1/0 actual1M model A/B/A pending; exact38 GPU operator gates passed',production_promoted=False,full1m_candidate_qualified=False,actual1m_input_completed_this_round=False,ordinary_decode_gain_proven=False,concurrent_identity_fixed=False)
roles['publication']['evidence_revision']='ttft-1m-20261025/r10'
sp.write_text(json.dumps(source,indent=2,ensure_ascii=False)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(dict(retained=True,revision='r10',new_files=sum(p.is_file() for p in D.rglob('*')),prior_entries_preserved=len(old),total_manifest_entries=len(manifest['files']),operator_GPU_cases=38,model_gain_qualified=False,formal_C13_restored=True,goal_complete=False)))
