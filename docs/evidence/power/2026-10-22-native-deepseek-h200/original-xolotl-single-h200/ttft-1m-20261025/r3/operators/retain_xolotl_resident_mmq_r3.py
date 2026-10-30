"""Retain an immutable resident-MMQ round and exact stable-rollover scope."""
import hashlib
import json
import subprocess
import time
from pathlib import Path

B=Path('D:/code/aster/target/h200-native')
W=Path('D:/code/aster/.wt-h200-native-integration')
E=W/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200'
D=E/'ttft-1m-20261025/r3'
if D.exists():raise RuntimeError('R3 already retained; use another immutable revision')
controller=json.loads((B/'xolotl-resident-compare-v37-controller.json').read_text())
if controller['phase']!='passed':raise RuntimeError('all three same-card profiles must complete first')
gate=json.loads((B/'xolotl-resident-mmq-gpu-gate-v36r2-latest.json').read_text())
if gate['result']['phase']!='passed' or gate['exact_process_alive']:raise RuntimeError('corrected exact gate must pass and finish')
secrets=[]
for p in B.glob('access-private-*/api-key.txt'):
    raw=p.read_bytes().strip()
    if len(raw)>=24 and raw not in secrets:secrets.append(raw)
if not secrets:raise RuntimeError('actual credential negative controls missing')
D.mkdir(parents=True)
files=[]

def put(raw,name,source=None):
    if len(raw)>2*1024**2 or any(k in raw for k in secrets):raise RuntimeError('public source size/credential gate failed: '+name)
    p=D/name
    p.parent.mkdir(parents=True,exist_ok=True)
    with p.open('xb') as f:f.write(raw)
    files.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),source=source))

def copy(name,dest):put((B/name).read_bytes(),dest,name)

def data(v,name):put((json.dumps(v,indent=2,ensure_ascii=False)+'\n').encode(),name)

for folder in ['resident-mmq-v35','resident-mmq-v36r2']:
    for p in sorted((B/folder).iterdir()):
        if p.is_file():copy(folder+'/'+p.name,'frozen-candidates/'+folder+'/'+p.name)
receipts=[
    'xolotl-resident-mmq-build-v35-intent.json','xolotl-resident-mmq-build-v35-latest.json',
    'xolotl-resident-mmq-gpu-gate-v36-intent.json','xolotl-resident-mmq-gpu-gate-v36-latest.json',
    'xolotl-resident-mmq-gpu-gate-v36r2-intent.json','xolotl-resident-mmq-gpu-gate-v36r2-latest.json',
    'xolotl-resident-compare-v37-controller.json','xolotl-stable-refresh-v32-qualification-latest.json',
    'xolotl-resident-mmq-v35-build-plan.json','xolotl-resident-v37-profile-configs.json',
    'xolotl-stable-refresh-over128k-v33-latest.json',
    'xolotl-two-role-v31-restoration-terminal.json','xolotl-windows-models-v34-latest.json',
]
rounds=[]
for profile in ['v15_control','v35_keep','v35_packed']:
    name='xolotl-resident-compare-v37-'+profile+'-qualification-latest.json'
    q=json.loads((B/name).read_text())
    if q['result']['phase']!='passed' or q['exact_client_alive']:raise RuntimeError('terminal28 request comparison required')
    tests={row['name']:row for row in q['result']['tests']}
    rounds.append(dict(profile=profile,requests=len(tests),short=q['result']['short_summary'],long={n:dict(ttft_seconds=tests[n]['ttft_seconds'],seconds=tests[n]['seconds'],usage=tests[n]['usage']) for n in ['cold35700','restored35700','mixed-short','mixed-cold-long','fresh-three-records','yielded-isolation-53','yielded-three-records']},three_position_retrieval_passed=q['result']['handoff_three_position_retrieval_passed']))
    receipts.append(name)
    for suffix in ['latest','diagnostic','drain']:
        n='xolotl-resident-compare-v37-'+profile+'-'+suffix+'.json'
        if (B/n).exists():receipts.append(n)
for name in receipts:copy(name,'receipts/'+name)
for kind in ['remainder','fullwindow']:
    for suffix in ['intent','latest']:
        n='xolotl-resident-v38-'+kind+'-'+suffix+'.json'
        if (B/n).exists():copy(n,'receipts/'+n)
operators=[
    'prepare_xolotl_resident_mmq_v35.py','launch_xolotl_resident_mmq_build_v35.frozen.py',
    'observe_xolotl_resident_mmq_build_v35.py','launch_xolotl_resident_mmq_gpu_gate_v36.frozen.py',
    'launch_xolotl_resident_mmq_gpu_gate_v36r2.frozen.py','observe_xolotl_resident_mmq_gpu_gate_v36r2.py',
    'launch_xolotl_resident_compare_service_v37.frozen.py','manage_xolotl_resident_compare_v37r1.frozen.py',
    'manage_xolotl_resident_compare_v37.frozen.py','run_xolotl_resident_compare_v37.frozen.py',
    'launch_xolotl_resident_fullwindow_v38.frozen.py','observe_xolotl_resident_fullwindow_v38.frozen.py',
    'xolotl_resident_qos_remainder_worker_v38.frozen.py','xolotl_ttft_qualification_worker_v15.frozen.py',
    'xolotl_context_1m_worker_v8.frozen.py','xolotl_full_1m_qualification_worker_v17.frozen.py',
    'launch_xolotl_stable_refresh_v31.frozen.py','observe_xolotl_stable_refresh_v31.py',
    'launch_xolotl_stable_refresh_qualification_v32.frozen.py','launch_xolotl_stable_refresh_over128k_v33.frozen.py',
    'promote_xolotl_stable_refresh_v31.frozen.py','finalize_xolotl_stable_refresh_v31_promotion.frozen.py',
    'finalize_xolotl_stable_refresh_v31_promotion_v2.frozen.py','launch_xolotl_server_vip_cleanup_v31.frozen.py',
    'submit_xolotl_single1m_cont7_lease.frozen.py','xolotl_trial_common.py','xolotl_serving_common.py',
    'launch_xolotl_longctx_shadow_v1.py','prepare_xolotl_resident_mmq_r3_outcome.py','retain_xolotl_resident_mmq_r3.py',
]
for name in operators:
    if (B/name).exists():copy(name,'operators/'+name)
stable=json.loads((B/'xolotl-stable-service-v31-intent.json').read_text())
promotion=json.loads((B/'xolotl-stable-refresh-v31-promotion.json').read_text())
promotion_fields=['unix','phase','base_url','old_allocation_uid','new_allocation_uid','context_tokens','actual_prompt_tokens_qualified','full_1m_length_qualified','full1m_cache_restore_passed','native_tool_arguments_repaired','workbuddy_gui_end_to_end_passed','same_existing_key','lease_ends_unix','endpoint_object_read_permitted','endpoint_read_failure','backend_verified_by_unique_nonce_and_kv_reuse','target_pod_prime','finished_unix','windows_forwarding','windows_direct_health','windows_direct_generation','post_switch_service','switch']
data({k:promotion[k] for k in promotion_fields if k in promotion},'receipts/xolotl-stable-refresh-v31-promotion-public.json')
public=dict(unix=time.time(),base_url='http://10.102.93.68/v1',model='deepseek-v4.1-flash-1017-xq',allocation_uid=stable['allocation_uid'],replica_uid=stable['replica_uid'],gpu_uuid=stable['gpu_uuid'],binary_sha256=stable['plan']['binary_sha256'],context_tokens=1048576,actual_prompt_tokens_qualified=132097,full_1m_length_qualified=False,instance_full_1m_execution_tested=False,engine_profile_full_1m_length_qualified=True,engine_profile_full1m_evidence_gpu='GPU-117aafbe-8432-d158-0666-42464fe5ff49',engine_profile_full1m_evidence='../r2/receipts/xolotl-full-1m-qualification-v17-latest.json',lease_ends_unix=stable['plan']['lease_ends_unix'],parallel=2,prefill_batch_tokens=8192,prefill_chunk_tokens=128,conversation_cache_mib=8192,conversation_cache_slots=8,windows_forwarding=False,same_existing_key=True,credential_retained=False,native_tool_arguments_repaired=False,workbuddy_gui_end_to_end_passed=False,resident_mmq_promoted=False,exact_tokenizer_memo_enabled=False,backend_verified_by_unique_nonce_and_kv_reuse=promotion['backend_verified_by_unique_nonce_and_kv_reuse'],endpoint_object_read_permitted=False)
data(public,'public-service-v31.json')
observation=json.loads((B/'xolotl-trial-resource-latest.json').read_text())
proof=observation['resource-result.json']
data(dict(unix=observation['unix'],job=observation['job'],job_uid=observation['job_uid'],phase=observation['phase'],replicas=[dict(name=r['name'],uid=r['uid'],phase=r['phase'],pod_ip=r['status'].get('podIP')) for r in observation['replicas']],expected_gpus=1,gpus=proof['gpus'],cuda=proof['cuda'],resource_proof_phase=proof['phase'],lease_ends_unix=proof['leaseEndsUnix'],maximum_task_owned_gpus=2),'optimization-resource-cont7.json')
data(dict(schema='aster-xolotl-resident-mmq-r3',unix=time.time(),rounds=rounds,gate=gate['result'],api_results_are_same_physical_gpu=True,weight_bytes_changed=False,routing_changed=False,new_gpu_kernels=False,additional_vram_buffers=False,production_promoted=False,full1m_candidate_qualified=False,credential_occurrences=0),'round-summary.json')
copy('resident-mmq-r3-OUTCOME.md','OUTCOME.md')
data(dict(unix=time.time(),schema='aster-xolotl-resident-mmq-r3-manifest',files=files),'manifest.json')
manifest_path=E/'manifest.json'
m=json.loads(manifest_path.read_text())
existing={row['file'] for row in m['files']}
for row in m['files']:
    raw=(E/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('published historical evidence changed')
for p in sorted(D.rglob('*')):
    if not p.is_file():continue
    name=p.relative_to(E).as_posix()
    if name in existing:raise RuntimeError('R3 evidence collides with historical evidence')
    raw=p.read_bytes();m['files'].append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
m['latest_resident_mmq_revision']='ttft-1m-20261025/r3'
manifest_path.write_text(json.dumps(m,indent=2)+'\n',newline='\n')
source_path=W/'docs/source-manifest.json'
s=json.loads(source_path.read_text())
v=s['original_xolotl_h200_20261024']
v['server_access_1m'].update(active_service=public,evidence='docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200/ttft-1m-20261025/r3',full_1m_length_qualified=False,engine_profile_full_1m_length_qualified=True,instance_full_1m_execution_tested=False)
for name in ['stable_serving','server_access_131k']:
    v[name].update(historical_snapshot=True,superseded_by='server_access_1m')
parent_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()
v['current_serving_optimization_roles']=dict(unix=time.time(),goal_objective='深度完成单卡H200 DeepSeek v4.1 Flash 1M上下文 三值量化版本模型推理速度优化',goal_met=False,active_service=public,optimization=dict(allocation_uid=observation['job_uid'],gpu_uuid=proof['gpus'][0]['uuid'],lease_ends_unix=proof['leaseEndsUnix'],current_profile='v35_packed',binary_sha256='fedf3b4199ec2c34915ca86b40f53656e27282763f34fa5eb0883b42e649eedd',production_promoted=False,finite_basic_comparison_completed=True,full1m_candidate_qualified=False),maximum_allocated_h200=2,publication=dict(parent_head_before_r3=parent_head,evidence_revision='ttft-1m-20261025/r3'),next_action='Observe the exact packed V38 remainder/fullwindow clients; keep formal V31 unchanged until actual full-window correctness and speed qualification. Profile dense/indexer GPU work next; shorter35k prefill does not prove full1M speed or faster decode. Tool arguments and WorkBuddy acceptance remain open.')
v['latest_resident_mmq_candidate']=dict(evidence='docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200/ttft-1m-20261025/r3',binary_sha256='fedf3b4199ec2c34915ca86b40f53656e27282763f34fa5eb0883b42e649eedd',production_promoted=False,real_weight_f32_gate_passed=True,api_same_gpu_28_request_profiles=rounds,full1m_candidate_qualified=False)
source_path.write_text(json.dumps(s,indent=2,ensure_ascii=False)+'\n',newline='\n')
print(json.dumps({'phase':'retained','r3_files':sum(p.is_file() for p in D.rglob('*')),'historical_files_verified':len(existing),'credential_occurrences':0,'production_promoted':False}))
