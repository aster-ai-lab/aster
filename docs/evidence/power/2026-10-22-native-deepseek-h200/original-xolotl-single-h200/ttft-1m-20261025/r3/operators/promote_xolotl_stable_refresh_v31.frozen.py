"""Promote only the actual full-window qualified V15; keep the same VIP/key."""
import ast, json, time
from xolotl_trial_common import BASE, jump, observe

ip = BASE / 'xolotl-stable-refresh-v31-promotion.json'
if ip.exists():
    raise RuntimeError('V15 promotion exists; inspect the existing switch instead of repeating it')
target = json.loads((BASE / 'xolotl-stable-refresh-v31-intent.json').read_text())
old = json.loads((BASE / 'xolotl-stable-service-v15-intent.json').read_text())
full = json.loads((BASE / 'xolotl-full-1m-qualification-v17-latest.json').read_text())
load = json.loads((BASE / 'xolotl-fullwindow-reuse-load-v20-latest.json').read_text())
observation = observe()
if observation['job_uid'] != target['allocation_uid'] or observation['phase'] != 'Running':
    raise RuntimeError('fresh qualified target allocation differs')
for item in [full, load]:
    if item['request-result.json']['phase'] != 'passed' or item['exact_client_alive']:
        raise RuntimeError('full-window correctness/load clients must be passed and gone')
if (not full['request-result.json'].get('full1m_cache_restore_passed') or
        full['request-result.json']['actual_prompt_tokens'] != 1048504 or
        load['request-result.json']['measured_requests'] != 32 or
        [row['concurrency'] for row in load['request-result.json']['levels']] != [1, 2]):
    raise RuntimeError('actual full-window correctness, restore or load coverage differs')
instance=json.loads((BASE/'xolotl-stable-refresh-v32-qualification-latest.json').read_text())
edge=json.loads((BASE/'xolotl-stable-refresh-over128k-v33-latest.json').read_text())
if instance['request-result.json']['phase']!='passed' or instance['exact_client_alive'] or len(instance['request-result.json']['tests'])!=28:raise RuntimeError('new instance matched28-request QA must pass and finish')
edge_q=edge['request-result.json']
if edge_q['phase']!='passed' or edge['exact_client_alive'] or edge_q['usage']['prompt_tokens']!=132097 or edge_q['content'].strip()!='42' or not edge_q['done']:raise RuntimeError('new instance132097-token actual regression must pass')
if target['plan']['binary_sha256']!=old['plan']['binary_sha256'] or target['plan']['engine_env']!=old['plan']['engine_env']:raise RuntimeError('refresh must preserve exact fully1M-qualified backend/profile')
new_lease = json.loads((BASE / 'xolotl-current-optimization-lease.json').read_text())
old_lease = json.loads((BASE / 'xolotl-current-serving-lease.json').read_text())
if new_lease['submitted']['uid'] != target['allocation_uid'] or old_lease['submitted']['uid'] != old['allocation_uid']:
    raise RuntimeError('exact old/new role lease identities differ')
with (BASE / 'xolotl-serving-v15-frozen-lease-v31.json').open('x') as stream:
    stream.write(json.dumps(old_lease, indent=2) + '\n')
source = (BASE / 'xolotl_serving_common.py').read_text()
if source.count('xolotl-current-serving-lease.json') != 1:
    raise RuntimeError('explicit current serving transport alias differs')
frozen = source.replace('xolotl-current-serving-lease.json', 'xolotl-serving-v15-frozen-lease-v31.json')
frozen = frozen.replace('xolotl-serving-resource-latest.json', 'xolotl-v15-serving-frozen-resource-v31.json')
ast.parse(frozen)
with (BASE / 'xolotl_serving_v15_frozen_v31.py').open('x', encoding='utf-8') as stream:
    stream.write(frozen)
record = dict(unix=time.time(), phase='prepared', base_url='http://10.102.93.68/v1',
              old_allocation_uid=old['allocation_uid'], new_allocation_uid=target['allocation_uid'],
              context_tokens=1048576, actual_prompt_tokens_qualified=132097,
              full_1m_length_qualified=False, full1m_cache_restore_passed=False,
              native_tool_arguments_repaired=False, workbuddy_gui_end_to_end_passed=False,
              same_existing_key=True, lease_ends_unix=target['plan']['lease_ends_unix'],
              full_context_qualification=full['request-result.json'],
              full_context_load={key: value for key, value in load['request-result.json'].items() if key != 'tests'})
with ip.open('x') as stream:
    stream.write(json.dumps(record, indent=2) + '\n')
record.update(engine_profile_full_1m_length_qualified=True,engine_profile_full1m_evidence_gpu=old['gpu_uuid'],instance_full_1m_execution_tested=False,instance_actual_prompt_tokens_qualified=132097,instance_qualification=instance['request-result.json'],instance_remainder_qualification=edge_q,qualification_scope='Same exact V15 executable/env; actual1M engine-profile proof on priorGPU, fresh instance tested132097; no fresh-instance1M timing claim')
ip.write_text(json.dumps(record,indent=2)+'\n')
reservation = BASE / 'xolotl-active-serving-reservation.json'
with (BASE / 'xolotl-pre-v31-serving-reservation.frozen.json').open('xb') as stream:
    stream.write(reservation.read_bytes())
reservation.write_text(json.dumps(dict(schema='aster-serving-reservation-v1', unix=time.time(),
                                      allocation_uid=target['allocation_uid'], gpu_uuid=target['gpu_uuid'],
                                      purpose='actual full-window qualified public serving; optimization prohibited',
                                      old_serving_allocation_to_drain=old['allocation_uid']), indent=2) + '\n')
code = '''import json,shlex,subprocess,time,urllib.request
from pathlib import Path
target=@TARGET@;old=@OLD@
def cli(args,data=None):
 p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl']+args)],input=data,capture_output=True,text=True,timeout=20)
 if p.returncode:raise RuntimeError('VIP CLI failed: '+p.stderr[-1500:])
 if len(p.stdout)>1048576:raise RuntimeError('VIP result bound exceeded')
 return json.loads(p.stdout)
for kind,name,uid in [('rjob',target['allocation_job'],target['allocation_uid']),('replica',target['replica'],target['replica_uid'])]:
 item=cli(['get',kind,name,'-n','ailab-hrprdduanyan','-o','json'])
 if item['metadata']['uid']!=uid or item['status']['phase']!='Running':raise RuntimeError('fresh V15 target identity differs')
root=Path(target['root']);state=json.loads((root/'service-longctx-v1-result.json').read_text())
if state['phase']!='serving' or state['context_tokens']!=1048576 or state['binary_sha256']!=target['plan']['binary_sha256'] or time.time()+900>state['lease_ends_unix']:raise RuntimeError('qualified V15 service is not ready')
for path in ['qualification-stable-refresh-v32/request-result.json','qualification-over128k-v33/request-result.json']:
 if json.loads((root/path).read_text())['phase']!='passed':raise RuntimeError('fresh full-window public gates differ')
key=(root/'credentials/api-key.txt').read_text().strip();payload=dict(model=target['plan']['model_name'],messages=[dict(role='user',content='Calculate 17 + 25. Reply with only the integer answer.')],max_tokens=16,temperature=0,reasoning_effort='none',stream=False)
req=urllib.request.Request('http://'+target['pod_ip']+':30003/v1/chat/completions',data=json.dumps(payload).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=15) as response:control=json.load(response)
if control['choices'][0]['message']['content'].strip()!='42':raise RuntimeError('live V15 pre-switch actual generation failed')
before=cli(['get','service','aster-xolotl-api-v3pool-20261024','-n','ailab-hrprdduanyan','-o','json'])
if before['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or before['spec']['selector']!={'rjob.brainpp.cn/rjob-name':old['allocation_job']} or before['metadata']['annotations']['aster.serving/allocation-uid']!=old['allocation_uid'] or before['status']['loadBalancer']['ingress']!=[dict(ip='10.102.93.68')]:raise RuntimeError('existing VIP owner/backend differs')
after=json.loads(json.dumps(before));after.pop('status',None);after['metadata'].pop('managedFields',None)
after['metadata']['annotations']['aster.serving/allocation-uid']=target['allocation_uid']
after['metadata']['annotations']['aster.serving/lease-ends-unix']=str(target['plan']['lease_ends_unix'])
after['metadata']['ownerReferences']=[dict(apiVersion='rjob.brainpp.cn/v1alpha1',kind='RJob',name=target['allocation_job'],uid=target['allocation_uid'],controller=False,blockOwnerDeletion=False)]
after['spec']['selector']={'rjob.brainpp.cn/rjob-name':target['allocation_job']}
result=cli(['apply','-f','-','-o','json'],json.dumps(after))
if result['metadata']['uid']!=before['metadata']['uid'] or result['spec']['clusterIP']!=before['spec']['clusterIP'] or result['spec']['selector']!=after['spec']['selector']:raise RuntimeError('exact V15 VIP switch result differs')
print(json.dumps(dict(unix=time.time(),before=before,after=result,resource_version_precondition=before['metadata']['resourceVersion'],pre_switch_generation='42')))
'''.replace('@TARGET@', repr(target)).replace('@OLD@', repr(old))
compile(code, 'actual-full-window-V15-owned-VIP-promotion', 'exec')
record['phase'] = 'switch_requested'
ip.write_text(json.dumps(record, indent=2) + '\n')
record['switch'] = jump(code)
record['phase'] = 'switched'
ip.write_text(json.dumps(record, indent=2) + '\n')
(BASE / 'xolotl-current-serving-lease.json').write_text(json.dumps(new_lease, indent=2) + '\n')
target.update(phase='serving', promoted=True, promotion=ip.name, actual_prompt_tokens_qualified=132097,
              full_1m_length_qualified=False, full1m_cache_restore_passed=False,
              native_tool_arguments_repaired=False, workbuddy_gui_end_to_end_passed=False)
target.update(engine_profile_full_1m_length_qualified=True,instance_full_1m_execution_tested=False,engine_profile_full1m_evidence_gpu=old['gpu_uuid'],instance_actual_prompt_tokens_qualified=132097)
(BASE / 'xolotl-stable-service-v31-intent.json').write_text(json.dumps(target, indent=2) + '\n')
print(json.dumps(dict(phase='switched', same_existing_key=True, base_url=record['base_url'],
                      full_1m_length_qualified=False, external_windows_validation_pending=True)))
