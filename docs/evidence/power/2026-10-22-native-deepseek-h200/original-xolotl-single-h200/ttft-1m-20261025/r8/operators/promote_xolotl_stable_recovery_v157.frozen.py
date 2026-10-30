"""Recover the existing owned VIP after exact fresh-instance gates; preserve its key."""
import json
import time
from xolotl_recovery_common_v157 import BASE, jump, observe

path = BASE / 'xolotl-stable-recovery-v157-promotion.json'
if path.exists():
    raise RuntimeError('recovery promotion has an exact record; inspect it without repeating the switch')
target = json.loads((BASE / 'xolotl-stable-recovery-v157-stable-intent.json').read_text())
old = json.loads((BASE / 'xolotl-stable-service-v97-intent.json').read_text())
baseline = json.loads((BASE / 'xolotl-indexer-service-v44-intent.json').read_text())
full = json.loads((BASE / 'xolotl-indexer-fullwindow-suite-v45-latest.json').read_text())
basic = json.loads((BASE / 'xolotl-stable-recovery-v157-stable-qualification-latest.json').read_text())
extended = json.loads((BASE / 'xolotl-stable-extended-v157-stable-latest.json').read_text())
edge = json.loads((BASE / 'xolotl-stable-edge-suite-v157-latest.json').read_text())
if (basic['result']['phase'] != 'passed' or basic['exact_client_alive'] or len(basic['result']['tests']) != 28
        or extended['result']['phase'] != 'passed' or extended['exact_client_alive']
        or not extended['result']['native_tool_roundtrip_passed']
        or not extended['result']['exact_long_output_reference_matched']
        or len(extended['result']['tests']) != 12
        or edge['suite-result.json']['phase'] != 'passed' or edge['exact_client_alive']
        or edge['remainder/request-result.json']['usage']['prompt_tokens'] != 132097
        or not full['fullwindow/request-result.json']['full1m_cache_restore_passed']):
    raise RuntimeError('exact28/native-tool/matched1024/132097/profile1M gates incomplete')
expected_env = dict(baseline['plan']['engine_env'], ASTER_XQ_EXACT_TOKEN_CACHE='1')
if (target['plan']['binary_sha256'] != baseline['plan']['binary_sha256']
        or target['plan']['engine_env'] != expected_env
        or target['plan']['prefill_chunk_tokens'] != 128):
    raise RuntimeError('same actual1M native profile plus qualified exact API overlay required')
resource = observe()
if resource['phase'] != 'Running' or resource['job_uid'] != target['allocation_uid']:
    raise RuntimeError('fresh exact replacement resource differs')
record = dict(unix=time.time(), phase='prepared', base_url='http://10.102.93.68/v1',
    old_allocation_uid=old['allocation_uid'], new_allocation_uid=target['allocation_uid'],
    same_existing_key=True, context_tokens=1048576, instance_actual_prompt_tokens_qualified=132097,
    engine_profile_full_1m_length_qualified=True, engine_profile_evidence='xolotl-indexer-fullwindow-suite-v45-latest.json',
    instance_full_1m_execution_tested=False, native_tool_roundtrip_passed=True,
    workbuddy_gui_e2e_passed=False, speed_300_qualified=False, lease_ends_unix=target['plan']['lease_ends_unix'])
code = '''import json,shlex,subprocess,time,urllib.request
from pathlib import Path
target=@TARGET@;old=@OLD@
def cli(args,data=None):
 p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl']+args)],input=data,capture_output=True,text=True,timeout=20)
 if p.returncode:raise RuntimeError('exact owned VIP CLI failed: '+p.stderr[-1500:])
 if len(p.stdout)>1048576:raise RuntimeError('bounded owned VIP metadata exceeded')
 return json.loads(p.stdout)
for r,want in [(old,'terminal'),(target,'Running')]:
 for kind,name,uid in [('rjob',r['allocation_job'],r['allocation_uid']),('replica',r['replica'],r['replica_uid'])]:
  item=cli(['get',kind,name,'-n','ailab-hrprdduanyan','-o','json'])
  if item['metadata']['uid']!=uid:raise RuntimeError('fresh exact recovery allocation UID differs')
  phase=item['status']['phase']
  if (want=='terminal' and phase not in ['Succeeded','Failed','Stopped']) or (want=='Running' and phase!='Running'):raise RuntimeError('fresh exact recovery allocation state differs')
root=Path(target['root']);state=json.loads((root/'service-longctx-v1-result.json').read_text())
if state['phase']!='serving' or state['gpu_uuid']!=target['gpu_uuid'] or state['binary_sha256']!=target['plan']['binary_sha256'] or time.time()+900>state['lease_ends_unix']:raise RuntimeError('exact qualified replacement is not ready')
for folder in ['qualification-basic-v157','qualification-extended-v157','qualification-stable-edge-suite-v157/remainder']:
 q=json.loads((root/folder/'request-result.json').read_text())
 if q['phase']!='passed':raise RuntimeError('fresh qualified replacement receipt differs')
key=(root/'credentials/api-key.txt').read_text().strip()
body=dict(model=target['plan']['model_name'],messages=[dict(role='user',content='Calculate 17 + 25. Reply with only the integer answer.')],max_tokens=16,temperature=0,reasoning_effort='none',stream=False)
req=urllib.request.Request('http://'+target['pod_ip']+':30003/v1/chat/completions',data=json.dumps(body).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=15) as response:control=json.load(response)
if control['choices'][0]['message']['content'].strip()!='42':raise RuntimeError('actual replacement pre-switch generation failed')
before=cli(['get','service','aster-xolotl-api-v3pool-20261024','-n','ailab-hrprdduanyan','-o','json'])
if before['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or before['spec']['selector']!={'rjob.brainpp.cn/rjob-name':old['allocation_job']} or before['metadata']['annotations']['aster.serving/allocation-uid']!=old['allocation_uid'] or before['status']['loadBalancer']['ingress']!=[dict(ip='10.102.93.68')]:raise RuntimeError('failed service VIP/owner/selector differs')
after=json.loads(json.dumps(before));after.pop('status',None);after['metadata'].pop('managedFields',None)
after['metadata']['annotations']['aster.serving/allocation-uid']=target['allocation_uid']
after['metadata']['annotations']['aster.serving/lease-ends-unix']=str(target['plan']['lease_ends_unix'])
after['metadata']['ownerReferences']=[dict(apiVersion='rjob.brainpp.cn/v1alpha1',kind='RJob',name=target['allocation_job'],uid=target['allocation_uid'],controller=False,blockOwnerDeletion=False)]
after['spec']['selector']={'rjob.brainpp.cn/rjob-name':target['allocation_job']}
result=cli(['apply','-f','-','-o','json'],json.dumps(after))
if result['metadata']['uid']!=before['metadata']['uid'] or result['spec']['clusterIP']!=before['spec']['clusterIP'] or result['spec']['selector']!=after['spec']['selector']:raise RuntimeError('same owned recovery VIP switch result differs')
print(json.dumps(dict(unix=time.time(),before=before,after=result,resource_version_precondition=before['metadata']['resourceVersion'],pre_switch_generation='42')))
'''.replace('@TARGET@', repr(target)).replace('@OLD@', repr(old))
compile(code, 'owned-stable-recovery-v58-promotion', 'exec')
with path.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'switch_requested'
path.write_text(json.dumps(record, indent=2) + '\n')
record['switch'] = jump(code)
record['phase'] = 'switched'
path.write_text(json.dumps(record, indent=2) + '\n')
for name in ['xolotl-current-serving-lease.json', 'xolotl-active-serving-reservation.json']:
    with (BASE / (name.removesuffix('.json') + '-before-v157.frozen.json')).open('xb') as output:
        output.write((BASE / name).read_bytes())
(BASE / 'xolotl-current-serving-lease.json').write_bytes((BASE / 'xolotl-single1m-cont11-lease-pending.json').read_bytes())
(BASE / 'xolotl-active-serving-reservation.json').write_text(json.dumps(dict(
    schema='aster-serving-reservation-v1', unix=time.time(), allocation_uid=target['allocation_uid'],
    gpu_uuid=target['gpu_uuid'], purpose='qualified actual1M native profile public serving; optimization prohibited',
    previous_terminal_allocation_uid=old['allocation_uid']), indent=2) + '\n')
target.update(phase='serving', promoted=True, promotion=path.name, actual_prompt_tokens_qualified=132097,
    engine_profile_full_1m_length_qualified=True, instance_full_1m_execution_tested=False,
    engine_profile_full1m_evidence_gpu=baseline['gpu_uuid'], native_tool_arguments_repaired=True,
    native_tool_roundtrip_passed=True, workbuddy_gui_end_to_end_passed=False)
(BASE / 'xolotl-stable-service-v157-intent.json').write_text(json.dumps(target, indent=2) + '\n')
print(json.dumps(dict(phase='switched', base_url=record['base_url'], same_existing_key=True,
                      windows_validation_pending=True, native_tool_roundtrip_passed=True)))
