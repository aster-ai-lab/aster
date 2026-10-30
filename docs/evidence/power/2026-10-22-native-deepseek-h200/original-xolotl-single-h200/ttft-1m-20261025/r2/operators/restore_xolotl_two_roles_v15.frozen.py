"""Keep the new public V15 and move optimization only to a proven idle former server."""
import json, time
from xolotl_serving_v8_frozen_v15 import BASE, observe, owned

ip = BASE / 'xolotl-two-role-v15-restoration.json'
if ip.exists():
    raise RuntimeError('V15 role restoration exists; inspect it')
old = json.loads((BASE / 'xolotl-stable-service-v8-intent.json').read_text())
current = json.loads((BASE / 'xolotl-stable-service-v15-intent.json').read_text())
promotion = json.loads((BASE / 'xolotl-ttft-v15-promotion.json').read_text())
observation = observe()
if observation['job_uid'] != old['allocation_uid'] or observation['phase'] != 'Running' or promotion['phase'] != 'promoted':
    raise RuntimeError('former exact server or completed V15 promotion differs')
code = '''import json,subprocess,time
from pathlib import Path
r=@R@;state=json.loads((Path(r['root'])/'service-longctx-v1-result.json').read_text())
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid,gpu_uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=8).stdout.strip()
if state['phase']!='stopped' or not state.get('api_reaped') or active:raise RuntimeError('former API must be stopped/reaped and its GPU idle')
proof=r['process'];p=Path('/proc/'+str(proof['pid'])+'/stat')
if p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==proof['start_ticks']:raise RuntimeError('former exact serving supervisor remains live')
print(json.dumps(dict(unix=time.time(),former_service_state=state,gpu_apps=active,former_supervisor_gone=True)))
'''.replace('@R@', repr(old))
proof = owned(code)
lease = json.loads((BASE / 'xolotl-serving-v8-frozen-lease-v15.json').read_text())
reservation = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
if lease['submitted']['uid'] != old['allocation_uid'] or reservation['allocation_uid'] != current['allocation_uid']:
    raise RuntimeError('new public serving guard or former resource lease differs')
(BASE / 'xolotl-current-optimization-lease.json').write_text(json.dumps(lease, indent=2) + '\n')
import xolotl_trial_common as trial
fresh = trial.observe()
if fresh['job_uid'] != old['allocation_uid']:
    raise RuntimeError('fresh optimization alias resolves to the wrong allocation')
record = dict(unix=time.time(), phase='passed', maximum_physical_h200=2,
              serving=dict(allocation_uid=current['allocation_uid'], gpu_uuid=current['gpu_uuid'], intent='xolotl-stable-service-v15-intent.json',
                           context_tokens=1048576, actual_prompt_tokens_qualified=1048504, full_1m_length_qualified=True,
                           lease_ends_unix=current['plan']['lease_ends_unix']),
              optimization=dict(allocation_uid=old['allocation_uid'], gpu_uuid=old['gpu_uuid'], no_active_model_worker=True,
                                lease_ends_unix=old['plan']['lease_ends_unix']), drained_previous_service=proof)
ip.write_text(json.dumps(record, indent=2) + '\n')
path = BASE / 'xolotl-serving-and-optimization-next-iteration.json'
state = json.loads(path.read_text())
state['unix'] = time.time()
state['optimization'] = dict(record['optimization'], phase='idle', checked_cache_candidate='xolotl-mixed-keep-build-v19-latest.json',
                             gpu_byte_gate_compiled=True, gpu_byte_gate_passed=False,
                             needs_new_bounded_lease_for_full_1m_candidate=True)
state['next_action'] = 'V15 actual1M qualified public service is live onGPU117a, sameVIP/key and Windows SSE42 passed. FormerV8 GPU0349 stopped/reaped and now idle optimization alias. Run synthetic mixed-slot byte gate on idle optimization GPU, then renew that resource before a bounded full1M candidate run (current old lease ends00:37). Keep2H200 maximum. V19 host build and30 capacity cases pass; no GPU/model speed gain claimed yet. Exact1M tokenization CPU median.764s is a separate measured bottleneck.'
path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
path = BASE / 'active-worktree.json'
state = json.loads(path.read_text())
state.update(current_optimization_allocation_uid=old['allocation_uid'], current_optimization_gpu_uuid=old['gpu_uuid'],
             current_optimization_no_active_model_worker=True, current_original_engine_trial=None,
             current_original_engine_trial_phase='public_v15_serving_old_v8_drained_for_optimization',
             current_owned_allocation_intent=str(BASE / 'xolotl-current-optimization-lease.json'))
path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps(dict(phase='passed', serving_gpu=current['gpu_uuid'], optimization_gpu=old['gpu_uuid'], optimization_idle=True)))
