"""Append immutable seed-state comparisons, matched native trace and unique-source CPU fixture proof."""
import ast
import gzip
import hashlib
import json
import subprocess
import time
from pathlib import Path

B=Path('D:/code/aster/target/h200-native');W=Path('D:/code/aster/.wt-h200-native-integration')
E=W/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200';D=E/'ttft-1m-20261025/r7'
if D.exists() or subprocess.check_output(['git','status','--porcelain'],cwd=W):raise RuntimeError('immutable R7 requires an exclusive clean integration worktree')
def read(name):return json.loads((B/name).read_text(encoding='utf-8-sig'))

matrix=read('xolotl-pool-seed-v112r2-controller.json');cpu=read('xolotl-pool-seed-cpu-v105r2-latest.json')
link=read('xolotl-pool-seed-native-v106-latest.json');trace=read('xolotl-native-profile-v125-latest.json')
stats=read('xolotl-native-statistics-v126-latest.json');fixture=read('xolotl-code-fullwindow-fixture-v114-latest.json')
admit=read('xolotl-admission-seed-v103-numeric-summary.json');collection=read('xolotl-seed-round-public-v124.json')
if matrix['phase']!='comparison_completed' or len(matrix['profiles'])!=3 or any(not p['basic28_passed'] or not p['native_tool_long12_passed'] or not p['pool13_completed'] for p in matrix['profiles']):raise RuntimeError('exact159 pool-matrix requests must finish')
if cpu['exact_cpu_process_alive'] or cpu['result']['phase']!='compiled' or not cpu['result']['control_object_byte_equal'] or not cpu['result']['upstream_pool_suite_passed'] or [r['contracts'] for r in cpu['result']['unit_tests']]!=[113,113]:raise RuntimeError('exact CPU contracts/reproduction incomplete')
if link['exact_link_process_alive'] or link['result']['phase']!='linked' or not link['result']['control_binary_byte_equal'] or link['result']['invalid_startup_modes_rejected']!=8:raise RuntimeError('exact controlled native link incomplete')
if trace['exact_worker_alive'] or trace['result']['phase']!='complete' or not trace['result']['engine_reaped'] or not trace['result']['native_target_gone'] or trace['result']['native_info']['batch_slots']!='2':raise RuntimeError('exact matched batch2 trace incomplete')
if stats['exact_cpu_process_alive'] or stats['statistics-result.json']['phase']!='passed' or stats['numeric-summary.json']['parallel_slots']!=2:raise RuntimeError('exact matched numeric trace summary incomplete')
if fixture['exact_cpu_process_alive'] or fixture['prepare-result.json']['phase']!='passed' or fixture['fixture-metadata.json']['actual_prompt_tokens']!=1047544 or fixture['fixture-metadata.json']['corpus_repeated']:raise RuntimeError('exact unique-source CPU fixture incomplete')
keys=[]
for path in B.glob('access-private-*/api-key.txt'):
    value=path.read_bytes().strip()
    if len(value)>=24 and value not in keys:keys.append(value)
if not keys:raise RuntimeError('actual credential negative controls missing')
mp=E/'manifest.json';original=mp.read_bytes();manifest=json.loads(original);old_rows=list(manifest['files'])
for row in old_rows:
    raw=(E/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('historical evidence changed: '+row['file'])
D.mkdir(parents=True);files=[]
def put(raw,name,source=None):
    if len(raw)>2*1024**2 or any(k in raw for k in keys):raise RuntimeError('bounded public credential gate refused: '+name)
    if name.endswith('.gz'):
        plain=gzip.decompress(raw)
        if len(plain)>12*1024**2 or any(k in plain for k in keys):raise RuntimeError('decompressed public credential gate refused: '+name)
    if name.endswith('.py'):ast.parse(raw,filename=name)
    path=D/name;path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as output:output.write(raw)
    files.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),source=source))
def copy(name,target):put((B/name).read_bytes(),target,name)
def data(value,name):put((json.dumps(value,indent=2,ensure_ascii=False)+'\n').encode(),name)

patterns=['xolotl-admission-seed-*.json','xolotl-admission-service-v101-*.json','xolotl-admission-client-v102-*.json',
 'xolotl-admission-v101-*.json','xolotl-pool-seed-*.json','xolotl-pool-service-v108-*.json','xolotl-pool-extended-v108-*.json',
 'xolotl-pool-long-v111-*.json','xolotl-pool-controller-resume-*.json','xolotl-pool-launch-*.json','xolotl-pool-file-transport-*.json',
 'xolotl-code-fullwindow-fixture-v114-*.json','xolotl-code-fullwindow-campaign-v118-prepared.json','xolotl-nsys-cpu-protocol-v115-*.json',
 'xolotl-pool-runtime-v116.json','xolotl-native-profile-v117-*.json','xolotl-native-profile-v125-*.json',
 'xolotl-native-statistics-v123-*.json','xolotl-native-statistics-v126-*.json','xolotl-native-profile-v117-prepared.json',
 'xolotl-critical-path-v122-controller.json','xolotl-batch2-profile-v125-prepared.json','xolotl-seed-round-public-v124.json',
 'xolotl-natural-long-fixtures-v109-*.json','xolotl-stable-recovery-v97-promotion.json','xolotl-stable-v58-early-stop-v97-reason.json',
 'xolotl-stable-service-v97-intent.json','xolotl-single1m-cont10-lease-pending.json','xolotl-formal-health-v113.json',
 'xolotl-readiness-watch-v99-intent.json','xolotl-sealed-native-profile-*-intent.json']
for name in sorted({p.name for pattern in patterns for p in B.glob(pattern)}):copy(name,'receipts/'+name)
operators=['prepare_admission_seed_matrix_v101.py','launch_admission_seed_service_v101.frozen.py','manage_admission_seed_matrix_v102.frozen.py',
 'run_admission_seed_matrix_v103.frozen.py','admission_seed_native_worker_v98.frozen.py','launch_admission_seed_native_v98.frozen.py',
 'prepare_pool_seed_v104.py','launch_pool_seed_cpu_v105r2.frozen.py','pool_seed_native_worker_v106.frozen.py','launch_pool_seed_native_v106.frozen.py',
 'launch_pool_service_v108.frozen.py','manage_pool_basic_v108.frozen.py','manage_pool_extended_v108.frozen.py',
 'xolotl_natural_long_fixture_prepare_v109.frozen.py','stage_natural_long_fixture_v109.frozen.py','observe_natural_long_fixture_v109.frozen.py',
 'xolotl_pool_long_worker_v110.frozen.py','manage_pool_long_v111.frozen.py','run_pool_seed_comparison_v112.frozen.py',
 'prepare_pool_controller_resume_v112r1.py','run_pool_seed_comparison_v112r1.frozen.py','inspect_uncertain_pool_launch_v112r2.py',
 'recover_pool_launch_v112r2.frozen.py','prepare_pool_controller_resume_v112r2.py','run_pool_seed_comparison_v112r2.frozen.py',
 'prepare_pool_file_transport_v112r3.py','launch_pool_service_file_v112r3.frozen.py','recover_pool_handle_v112r3.py',
 'xolotl_code_fullwindow_prepare_v114.frozen.py','stage_code_fullwindow_fixture_v114.frozen.py','observe_code_fullwindow_fixture_v114.frozen.py',
 'stage_nsys_cpu_protocol_v115.frozen.py','observe_nsys_cpu_protocol_v115.frozen.py','inspect_pool_runtime_v116.frozen.py',
 'prepare_native_profile_v117.py','xolotl_native_profile_worker_v117.frozen.py','launch_native_profile_v117.frozen.py',
 'launch_native_profile_v117r1.frozen.py','observe_native_profile_v117.frozen.py','xolotl_sealed_bootstrap_v120.py',
 'prepare_remaining_file_launches_v120.py','run_critical_path_v122.frozen.py','xolotl_profile_statistics_worker_v123.frozen.py',
 'manage_profile_statistics_v123.frozen.py','collect_seed_round_public_v124.frozen.py','prepare_batch2_profile_v125.py',
 'xolotl_native_profile_worker_v125.frozen.py','launch_native_profile_v125.frozen.py','observe_native_profile_v125.frozen.py',
 'xolotl_sealed_bootstrap_v125.py','prepare_matched_profile_statistics_v126.py','xolotl_profile_statistics_worker_v126.frozen.py',
 'manage_profile_statistics_v126.frozen.py','xolotl_trial_common.py','xolotl_serving_common.py','retain_xolotl_seed_and_trace_r7.py']
for name in operators:copy(name,'operators/'+name)
for folder in ['admission-seed-v93','pool-seed-v104','pool-seed-v104r1','pool-seed-v104r2']:
    for path in sorted((B/folder).rglob('*')):
        if path.is_file() and path.suffix in {'.cpp','.hpp','.patch','.json','.py','.txt'}:
            copy(path.relative_to(B).as_posix(),'frozen-candidates/'+path.relative_to(B).as_posix())
for row in collection['files']:
    path=B/'seed-round-public-v124'/row['file'];raw=path.read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact collected public artifact differs')
    put(raw,row['file'],'seed-round-public-v124/'+row['file'])
copy('public-code-corpus-v114.json.gz','code-fixture/public-code-corpus.json.gz')
on=next(p for p in matrix['profiles'] if p['profile']=='pool_on');controls=[p for p in matrix['profiles'] if p['profile']!='pool_on']
gains=[dict(case=c['case'],on_restored_seconds=c['restored_seconds'],control_restored_seconds=[p['cases'][i]['restored_seconds'] for p in controls],
    reduction_percent=[100*(1-c['restored_seconds']/p['cases'][i]['restored_seconds']) for p in controls]) for i,c in enumerate(on['cases'])]
summary=dict(schema='aster-xolotl-seed-and-trace-r7',unix=time.time(),pool_profiles=matrix['profiles'],pool_restored_latency=gains,
    pool_model_requests=159,pool_standard_requests_passed=120,pool_diagnostic_requests_completed=39,pool_identity_gate_passed=False,
    admission=admit,admission_requests_completed=30,admission_concurrent_identity_gate_passed=False,
    cpu_contracts_per_mode=113,cpu_modes=['release','asan_ubsan'],original_upstream_pool_tests_passed=True,
    native_parent_binary_reproduced=True,matched_native_trace=stats['numeric-summary.json'],
    unique_actual1m_fixture_cpu=fixture['fixture-metadata.json'],actual1m_gpu_input_completed_this_round=False,
    candidates_promoted=False,formal_service_changed_by_seed_candidates=False,maximum_authorized_h200=2,single_request300_met=False,goal_complete=False)
data(summary,'round-summary.json')
table=['| Profile | Short-input1024-output API token/s | Restored8244 seconds | Restored25671 seconds | Full-output identity gate |',
       '|---|---:|---:|---:|---|']
for p in matrix['profiles']:table.append(f"| {p['profile']} | {p['long_summary']['api_wall_completion_tok_s']:.6f} | {p['cases'][0]['restored_seconds']:.6f} | {p['cases'][1]['restored_seconds']:.6f} | {p['pool13_phase']} |")
outcome='''# DSpark seed recovery and current1M native critical path (R7)

DSpark already uses the checkpoint's own MTP draft stack. The admission
candidate retains prompt-tail seed rows for a later solo regenerate. Its
same-GPU/same-binary0/1/0 comparison completes30 requests. Post-admission
serial elapsed sums improve25.40%/29.90%; this statistic is not a shared
block wall or concurrent throughput. The slowed control prompt changes
with scheduling. All three original concurrent-identity suites still fail.

The pool candidate forwards optional auxiliary bytes through the existing
budgeted pool wrappers, storing strict shape/position/finite seed metadata.
Invalid metadata and partial rewinds discard the seed. The original GEN
object and parent native executable reproduce byte-for-byte before the
single-object replacement.113 contracts pass in release and again under
ASAN/UBSAN; the unchanged upstream pool suite passes. Both operator flags
default off, and eight invalid startup modes are refused per native link.
Initial duplicate-header and undersized-test-slab CPU failures are retained.

The same GPU099e/native9a684/API f062/S512/1Mconfigured/GPFS/cache2M profile
runs pool0/1/0repeat with admissionseed1. Each completes28 basic,12 strict
native-tool/matched1024 and13 forced-pool requests.120 standard requests
pass. All39 diagnostic calls finish structurally; all three full-output
identity suites fail. Every long request emits1024 actual output tokens.
Native logs prove real pool restoration and128 restored seed rows only
with the candidate on. Restored latency improves, but complete cold-reference
output hashes still differ. The candidate is not promoted.

'''+ '\n'.join(table)+'''

A local Windows atomic-replace refusal interrupted only the observer. The
exact existing model/client were resumed without relaunch. Two later long
inline exec handshakes failed before the remote model root existed. Exact
Running UID, absent root and idle GPU were proven; hash-checked file execution
of the same bootstrap then succeeded. This supports an inline transport-size
hypothesis; the server's handshake rejection reason was not captured. No GPU
allocation was added and no uncertain process was blindly duplicated.

The current1M/S512 native critical path is captured after three pre-trace
128-output controls. The first direct trace has no batch slots and is retained
as a separate diagnostic. A second trace derives --batch2 from the exact
API f062 argument function and requires INFO ctx1048576,batch_slots2,slot_cache1.
All four requests in each trace finish and both native targets are reaped.
The matched trace records188945 kernels,46029 copies and9610 stream-sync
calls. GPU-event union/span is0.654938. These are recorded events with
profiling overhead, not physical SM utilization, wall-time attribution,
API throughput or a hardware speed ceiling. Summed runtime waits overlap
GPU work. Raw Nsight reports/databases remain private; numeric summaries
and exact operators are retained.

The new CPU-qualified fixture contains1047544 input tokens, reserves1024
output plus8 safety, and quotes a continuous prefix of a1510935-token public
code corpus from262 unique pinned source files. No corpus cycling is used;
62 boundary-padding repetitions account for exact length. Reserved control
tokens are checked with the real tokenizer. Record positions are36,523698,
1047424. License, file hashes, source revision, original archive SHA, prompt
bytes/ID hashes and compressed reproducible public input are retained.
CPU preparation is not an actual1M GPU model result.

C8 stopped unexpectedly before its recorded lease; cause/actor is not proven.
The idle C9 was separately qualified28+12+strict132097, then recovered the
same VIP through an owned backend nonce and direct Windows generation. C10
followed exact C8 terminal/C9 Running ownership guards, preserving max2.
The formal C9 remains nativefa/S128; these seed candidates do not replace it.
Old WorkBuddy CLI proof belongs to the prior instance; fresh GUI and cold
first-turn latency acceptance are not claimed.

## Commands and counts

- run_admission_seed_matrix_v103.frozen.py:30 completed diagnostic requests;
  original concurrent identity gates retained and failed.
- launch_pool_seed_cpu_v105r2.frozen.py and launch_pool_seed_native_v106.frozen.py:
  exact parent reproduction,113x2 CPU contracts, upstream tests, invalid flags.
- run_pool_seed_comparison_v112*.frozen.py:159 model requests,120 standard
  passes and39 completed failed-identity diagnostics; all profiles drained.
- stage_code_fullwindow_fixture_v114.frozen.py: exact CPU tokenizer fixture,
  unchanged six completion-check unit methods, no GPU inference.
- launch_native_profile_v117r1.frozen.py / launch_native_profile_v125.frozen.py:
  four128-output requests each, independent exact processes and bounded cleanup.
- manage_profile_statistics_v123.frozen.py / v126: CPU-only numeric aggregation.
- retain_xolotl_seed_and_trace_r7.py: historical manifest verification, bounded
  actual-key negative controls, source/fixture/evidence retention. No Cargo run.

## Explicitly not claimed

- No300/1000 single-request achievement, goal completion or speed ceiling.
- No ordinary long-output decode gain: the three controls remain75–76token/s.
- No accepted complete-output pool/concurrency fix or seed-candidate promotion.
- No actual1M GPU input in these seed matrices/traces; new unique-source1M
  GPU validation follows separately. Configured1M is not actual1M input.
- No full DFlash/DeepInfer H200XQ service integration, standalone additional
  MTP multiplier, fresh WorkBuddy GUI pass or initial-latency acceptance.
- No weights, binaries, object files, credentials, private client state or
  raw profiler traces/databases; no new Rust workspace CI/fmt claim.
'''
put(outcome.encode(),'OUTCOME.md')
data(dict(schema='aster-xolotl-seed-trace-r7-manifest',unix=time.time(),files=files),'manifest.json')
existing={r['file'] for r in old_rows}
for path in sorted(D.rglob('*')):
    if path.is_file():
        name=path.relative_to(E).as_posix();raw=path.read_bytes()
        if name in existing:raise RuntimeError('immutable evidence collision')
        manifest['files'].append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
if manifest['files'][:len(old_rows)]!=old_rows:raise RuntimeError('historical manifest row order changed')
manifest.update(latest_seed_and_native_trace_revision='ttft-1m-20261025/r7',goal_complete=False)
newline='\r\n' if b'\r\n' in original else '\n';mp.write_bytes((json.dumps(manifest,indent=2)+'\n').replace('\n',newline).encode())
p=W/'docs/source-manifest.json';source=json.loads(p.read_text());entry=source['original_xolotl_h200_20261024'];relative=D.relative_to(W).as_posix()
entry['latest_dspark_seed_recovery']=dict(evidence=relative,admission_requests=30,pool_requests=159,standard_requests_passed=120,
    restored_latency_observed=gains,full_output_identity_gate_passed=False,production_promoted=False,ordinary_decode_gain_proven=False)
entry['latest_current_1m_native_trace']=dict(evidence=relative,context_tokens=1048576,prefill_chunk_tokens=512,parallel_slots=2,
    actual_prompt_tokens=stats['numeric-summary.json']['prompt_tokens'],actual1m_input_executed=False,
    raw_trace_published=False,kernel_count=188945,stream_synchronize_calls=9610,diagnostic_only=True)
entry['latest_unique_code_actual1m_fixture']=dict(evidence=relative,actual_input_tokens=1047544,max_output_tokens=1024,
    source_revision=fixture['fixture-metadata.json']['source_revision'],corpus_repeated=False,cpu_qualified=True,gpu_qualified=False)
current=read('xolotl-serving-and-optimization-next-iteration.json');formal=read('xolotl-stable-service-v97-intent.json')
public={k:current['active_service'][k] for k in ['base_url','model','allocation_uid','gpu_uuid','context_tokens','lease_ends_unix',
    'binary_sha256','api_sha256','prefill_batch_tokens','prefill_chunk_tokens','parallel','conversation_cache_mib','conversation_cache_slots',
    'windows_forwarding','actual_prompt_tokens_qualified','full_1m_length_qualified','instance_full_1m_execution_tested',
    'engine_profile_full_1m_length_qualified','same_existing_key','native_tool_arguments_repaired','workbuddy_gui_end_to_end_passed']}
public.update(allocation_job=formal['allocation_job'],replica_uid=formal['replica_uid'],credentials_retained=False,
    backend_verified_by_unique_nonce_and_kv_reuse=True,workbuddy_bundled_cli_e2e_passed=False,workbuddy_first_turn_latency_accepted=False)
roles=entry['current_serving_optimization_roles'];roles.update(unix=time.time(),active_service=dict(public),serving=dict(public),goal_met=False,
    maximum_allocated_h200=2,next_action='Use the matched1M/batch2 trace to target Q3/dense execution and synchronization; finish new unique-code actual1M GPU suite; preserve failed identity gates and formal C9; max2 GPUs.')
roles['optimization']=dict(allocation_uid='7455d989-8255-4f7c-ae8f-3cc0aa5928e4',gpu_uuid='GPU-099e8da4-8df0-c431-9e67-eb721e59beee',
    lease_ends_unix=1792910673.5259097,binary_sha256=link['result']['binary']['sha256'],latest_evidence=relative,
    current_profile='native9a684/S512/1Mconfigured; seed diagnostic rejected for complete-output identity; matched native trace completed',
    production_promoted=False,full1m_candidate_qualified=False,actual1m_input_completed_this_round=False,
    ordinary_decode_gain_proven=False,concurrent_identity_fixed=False)
roles['publication']['evidence_revision']='ttft-1m-20261025/r7'
p.write_text(json.dumps(source,indent=2,ensure_ascii=False)+'\n',encoding='utf-8',newline='\n')
p=W/'.gitattributes';rule='/docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200/ttft-1m-20261025/r7/frozen-candidates/**/*.patch -text whitespace=-blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol\n'
text=p.read_text()
if rule.strip() in text:raise RuntimeError('R7 patch-data attribute already exists')
p.write_text(text.rstrip()+'\n'+rule,encoding='utf-8',newline='\n')
print(json.dumps(dict(phase='retained',revision='r7',files=sum(p.is_file() for p in D.rglob('*')),historical_entries_preserved=len(old_rows),
    credential_occurrences=0,model_requests=159+30+8,actual1m_gpu_input_completed=False,candidates_promoted=False,goal_complete=False)))
