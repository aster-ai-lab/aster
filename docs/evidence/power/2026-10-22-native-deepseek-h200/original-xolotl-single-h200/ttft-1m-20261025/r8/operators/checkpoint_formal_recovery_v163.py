"""Record newly verified formal C11 while keeping the independent ongoing C10 work exact."""
import json
import time
from xolotl_trial_common import BASE
def read(name):return json.loads((BASE/name).read_text())
p=read('xolotl-stable-recovery-v157-promotion.json');r=read('xolotl-stable-service-v157-intent.json')
if p['phase']!='promoted' or not p['backend_verified_by_unique_nonce_and_kv_reuse']:raise RuntimeError('exact fresh C11 Windows/backend proof required')
path=BASE/'xolotl-serving-and-optimization-next-iteration.json';raw=path.read_bytes()
with (BASE/'xolotl-serving-and-optimization-before-v163.frozen.json').open('xb') as output:output.write(raw)
c=json.loads(raw);old=dict(c['active_service']);old.update(terminal_evidence='xolotl-stable-v97-early-stop-v156-reason.json',resource_phase='Stopped')
c.setdefault('historical_service_snapshots',[]).append(old)
c['active_service'].update(intent='xolotl-stable-service-v157-intent.json',allocation_uid=r['allocation_uid'],gpu_uuid=r['gpu_uuid'],
    lease_ends_unix=r['plan']['lease_ends_unix'],resource_phase='Running',last_health_status=200,current_availability='C11 same VIP/key freshly verified by Windows nonce generation/KV reuse after28+12+strict132097; read-only watch159 active',
    latest_readonly_health_proof='xolotl-stable-recovery-v157-promotion.json',availability_evidence='xolotl-stable-recovery-v157-promotion.json',
    service_changed_this_round=True,windows_direct_generation=p['windows_direct_generation'],readiness_observer='xolotl-readiness-watch-v159-intent.json',
    workbuddy_bundled_cli_e2e_passed=False,workbuddy_native_read_end_to_end_passed=False,workbuddy_gui_end_to_end_passed=False,
    actual_prompt_tokens_qualified=132097,instance_full_1m_execution_tested=False,full_1m_length_qualified=False,
    native_tool_arguments_repaired=True,tool_call_end_to_end_passed=True,
    tool_scope='Fresh C11 native tool roundtrip + standard28 + matched1024output12 + strict132097 + Windows backend nonce; no fresh WorkBuddy CLI/GUI claim')
extended=read('xolotl-stable-extended-v157-stable-latest.json')['result']
c['active_service']['fresh_short_input_1024_output_load']=extended['long_summary']
c['current_recovery'].update(phase='promoted',new_gpu_uuid=r['gpu_uuid'],vip_switched=True,windows_verified=True,
    new_service_intent='xolotl-stable-service-v157-intent.json',lease_ends_unix=r['plan']['lease_ends_unix'],
    promotion='xolotl-stable-recovery-v157-promotion.json',controller_session=None,unified_session=None)
c.update(unix=time.time(),goal_met=False,next_priority='Keep C11 fresh readiness observed; finish exact C10 same-native literal API actual1M comparison; preserve failed C2 gate and target native GPU/I/O work next')
c['next_required_actions'][0]='C11 formal promoted, sameVIP/key/1Mconfigured/fa/S128/f062. Fresh28+12+strict132097 andWindowsnonceKVreusepassed; profile1Mproof historical, thisinstanceactual1Mnotexecuted. Read-only watch159; exactleaseends '+str(r['plan']['lease_ends_unix'])+'. Current C11short-input8x1024 APIwall72.776967tok/s onGPUae6a; differentGPU fromC10SIMDmatrix, do notcallregression. Refreshformalstatus periodically; C9 unexpectedStopped at10:51CST, causeunknown.'
c['next_required_actions'][1]='C11 recovery complete. Keep reservation alias exactC11UID1cfa236c-40ba-408f-a45e-003e0261bdf5; genericoptimizationownedrefuses it. C10UID7455 remainsindependentand Running. Maximum2 taskownedH200. No experimentalcandidates promoted; WorkBuddyGUI/CLI freshunverified.'
path.write_text(json.dumps(c,indent=2,ensure_ascii=False)+'\n')
ap=BASE/'active-worktree.json';raw=ap.read_bytes()
with (BASE/'active-worktree-before-v163.frozen.json').open('xb') as output:output.write(raw)
a=json.loads(raw)
a.update(current_service_intent=str(BASE/'xolotl-stable-service-v157-intent.json'),current_serving_allocation_uid=r['allocation_uid'],
    current_serving_gpu_uuid=r['gpu_uuid'],current_service_lease_ends_unix=r['plan']['lease_ends_unix'],
    current_service_available=True,current_service_fully_verified=False,current_service_basic_chat_verified=True,
    h200_serving_intent='xolotl-stable-service-v157-intent.json',pending_service_intent=None,pending_serving_allocation_uid=None,
    h200_formal_recovery_controller=str(BASE/'xolotl-stable-recovery-v158-controller.json'),h200_formal_recovery_session=None,
    last_formal_readonly_health=str(BASE/'xolotl-stable-recovery-v157-promotion.json'),
    latest_current_service_load_test=str(BASE/'xolotl-stable-extended-v157-stable-latest.json'),
    workbuddy_bundled_cli_e2e_passed=False,workbuddy_native_read_e2e_passed=False,workbuddy_gui_e2e_passed=False,
    actual_prompt_tokens_qualified=132097,full_1m_length_qualified=False,instance_full_1m_execution_tested=False)
ap.write_text(json.dumps(a,indent=2)+'\n')
print(json.dumps(dict(phase='checkpointed',formal_available=True,formal_uid=r['allocation_uid'],optimization_preserved=True,
    native_tool_roundtrip_passed=True,windows_nonce_KV_reuse_passed=True,fresh1024_output_API_tok_s=extended['long_summary']['api_wall_completion_tok_s'],goal_complete=False)))
