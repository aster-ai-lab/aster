"""Preserve prior checkpoints and record completed proofs plus exact active recovery handles."""
import json
import time
from xolotl_trial_common import BASE

def read(name):
    return json.loads((BASE/name).read_text())
path=BASE/'xolotl-serving-and-optimization-next-iteration.json'
old=path.read_bytes()
with (BASE/'xolotl-serving-and-optimization-before-v153.frozen.json').open('xb') as output:
    output.write(old)
c=json.loads(old)
matrix=read('xolotl-phase-simd-v140r1-controller.json')
full=read('xolotl-code-fullwindow-suite-v119-latest.json')['fullwindow/request-result.json']
cpu=read('xolotl-literal-scan-cpu-v145r1-latest.json')['cpu-result.json']
resource=read('xolotl-single1m-cont11-lease-pending.json')
c.update(unix=time.time(),goal_turn_classification='PROGRESS',goal_met=False,
    latest_user_question='H200GPU的全部能力一定要发挥出来啊',
    goal_turn_progress='Completed real unique-code1M cold/restore/c1, retained c2 identity failure; completed120 same-GPU SIMD requests; exact literal CPU A/B/A9.15/1.65/9.13s; C9 unexpected terminal, exact replacement C11 submitted with C10 preserved',
    latest_evidence_revision='ttft-1m-20261025/r7',r8_publication_pending=True,parent_publication_main='cd9681f827deda996bac13c1c9b6f30f739527dc')
c['active_service'].update(resource_phase='Stopped',last_health_status=None,current_availability='Unavailable: exact C9 job/replica Stopped before recorded lease; Windows direct VIP health timed out; C11 recovery in progress',
    latest_readonly_health_proof='xolotl-stable-v97-early-stop-v156-reason.json',service_changed_this_round=False,
    workbuddy_bundled_cli_e2e_passed=False,workbuddy_gui_end_to_end_passed=False)
c['optimization'].update(current_candidate='Same native trace-control99f3/S512/GPFS/admission1/pool1 baseline actual1M API comparison; literal API pending same-card GPU gates; SIMD only small ordinary decode difference',
    candidate_binary_sha256='99f3eee8c7eeee54c304120db50e8e632a7c001f9ac90fadf0983040fef92c25',
    current_service_intent='xolotl-phase-simd-v139-control_b-intent.json',
    current_service_root=read('xolotl-phase-simd-v139-control_b-intent.json')['root'],
    comparison_controller_state='xolotl-literal-api-v151-controller.json',comparison_controller_unified_session=18258,
    comparison_controller_finished=False,current_profile='control fullwindow canonical1047544+1024, then literal API same native/same GPU',
    current_fullwindow_suite_intent='xolotl-literal-fullwindow-v150-control-intent.json',
    current_fullwindow_suite_process=read('xolotl-literal-fullwindow-v150-control-intent.json')['process'],
    current_fullwindow_suite_phase='running',current_fullwindow_suite_stage='fullwindow',current_fullwindow_suite_verified_alive=True,
    actual_full1m_client_intent='xolotl-literal-fullwindow-v150-control-intent.json',
    actual_full1m_client_phase='running',actual_full1m_prompt_tokens=1047544,actual_full1m_client_verified_alive=True,
    actual_full1m_client=read('xolotl-literal-fullwindow-v150-control-intent.json')['process'],
    current_basic_client='xolotl-phase-simd-v139-control_b-qualification-intent.json',
    current_basic_client_process=read('xolotl-phase-simd-v139-control_b-qualification-intent.json')['process'],
    current_basic_client_phase='passed',current_basic_client_verified_alive=False,
    full1m_candidate_qualified=False,formal_promoted=False,no_active_model_worker=False,
    unique_code1m_suite_v119_phase='failed',unique_code1m_c1=full['levels'][0],unique_code1m_c2_identity_passed=False,
    simd_matrix=matrix['profiles'],simd_actual1m_input_executed=False,
    literal_scan_cpu_A_B_A=cpu['runs'],literal_scan_cpu_ids_exact=True,literal_scan_gpu_qualified=False)
c['current_recovery']=dict(controller='xolotl-stable-recovery-v157-controller.json',unified_session=6063,
    phase='waiting_existing_resource',new_allocation_job=resource['submitted']['rjob'],new_allocation_uid=resource['submitted']['uid'],
    exact_predecessor_terminal=True,maximum_h200=2,optimization_restarted=False,vip_switched=False,windows_verified=False,
    reason='xolotl-stable-v97-early-stop-v156-reason.json')
c['current_findings'].update(regular_long_output_baseline_tok_s=75.50382248049063,
    best_next_direction='Target actual dense GPU work, Engram I/O waits and exact frontend scans; use real API canonical fixture; preserve current C10 comparison during C11 same-profile formal recovery',
    proof_limits='C2 full-output identity fails; SIMD actual1M untested; literal improvement CPU-only so far; no300; C11 service not yet qualified/promoted')
c['next_required_actions']=[
    'Observe exact C11 recovery controller6063 and xolotl-stable-recovery-v157-controller.json. C9 unexpectedlyStopped beforelease, causeunknown. C11 submittedUID1cfa236c-40ba-408f-a45e-003e0261bdf5. KeepC10 running, maximum2 GPUs. Newformal samefa/S128/f062, not experimental SIMD/literal API.',
    'After freshC11 basic28+native-tool/1024output12+strict132097 pass, promote once via promote_xolotl_stable_recovery_v157.frozen.py then finalize_xolotl_stable_recovery_v157.frozen.py; verify sameVIP/key with Windows unique backend nonce/KVreuse; refreshreadinesswatch andcheckpoint. Do notclaimGUIe2e.',
    'Observe exact optimization controller18258, xolotl-literal-api-v151-controller.json. It waits existingbaseline150control actual1047544+1024+c1/c2; preserves full-output identity failures, drains onlyidle ownedC10, then exactsame-native literal API149 basic28/extended12/full150. No blindrelaunch onobserverfailure.',
    'CPU141 showed direct-template metadataebe492 differs from actualprotectedAPIb442 by6IDvalues butsame1047544count/body/recordpositions. Canonicalfixture147 correctsmetadata only. Do notclaim native runtimeIDhash captured.',
    'CPU SIMD137 exhaustive786432cases per mode inclASANUBSAN passed, original/default0objectidentical.120GPU standardrequests allpassed:75.7429/76.3113/75.5038APIwalltok_s. Small0.75–1.07%observeddifference; actual1M SIMDuntested, noformalpromotion.',
    'Literal145r1 actualAPI A/B/A9.1538/1.6472/9.1267s, allIDs exact;15helper+25isolatedupstreammethods passed. Newadapter14814CPU suites bothflags passed, actualAPI canonicalIDs match. GPU/API benefit pending. Prior145 generic-test monkeypatch contamination failure retained.',
    'Retain/pushR8 completed actual-code1M20+remainder result,120SIMD calls, all CPU/source/negative evidence and recovery receipts.55bounded remote public artifacts collected146. Preserve existing2295manifest entries and actual-key negative controls. Rootdirty useredits untouched; noCargo/fmt/workspaceCI claim.',
    'Continue source-linked H200 GPU kernel/copy/sync work after current finite suites.154snapshot98%busy,1980MHzSM/3201MHzmem=currentmax,700Wlimit,noactivecurrentclockthrottle; ncuavailable/usr/local/bin/ncu. Snapshot is not SM efficiency or bandwidth ceiling.'
]
c['next_priority']=c['next_required_actions'][0]
path.write_text(json.dumps(c,indent=2,ensure_ascii=False)+'\n')
active=BASE/'active-worktree.json';raw=active.read_bytes()
with (BASE/'active-worktree-before-v153.frozen.json').open('xb') as output:
    output.write(raw)
a=json.loads(raw)
a.update(current_service_available=False,current_service_fully_verified=False,
    pending_service_intent=str(BASE/'xolotl-stable-recovery-v157-stable-intent.json'),pending_serving_allocation_uid=resource['submitted']['uid'],
    current_original_engine_trial=str(BASE/'xolotl-phase-simd-v139-control_b-intent.json'),
    current_original_engine_trial_phase='canonical_actual1m_baseline_for_literal_API_comparison',
    h200_current_comparison_controller=str(BASE/'xolotl-literal-api-v151-controller.json'),current_comparison_session=18258,
    h200_current_fullwindow_client='manage_literal_fullwindow_suite_v150.py observe --profile control; do not duplicate existing client',
    h200_current_build=str(BASE/'xolotl-native-phase-simd-v138-intent.json'),
    current_full1m_qualification='V119 unique-code cold/restore/c1passed, c2full-outputidentityfailed; V150canonical control running',
    h200_formal_recovery_controller=str(BASE/'xolotl-stable-recovery-v157-controller.json'),h200_formal_recovery_session=6063,
    last_formal_readonly_health=str(BASE/'xolotl-stable-v97-early-stop-v156-reason.json'),
    current_goal_turn_classification='PROGRESS',h200_goal_complete=False,latest_h200_capacity_proof='xolotl-h200-capacity-v154.json',
    current_pending_publication_revision='ttft-1m-20261025/r8')
active.write_text(json.dumps(a,indent=2)+'\n')
print(json.dumps(dict(phase='checkpointed',formal_available=False,replacement_UID=resource['submitted']['uid'],optimization_controller_session=18258,recovery_controller_session=6063,goal_complete=False)))
