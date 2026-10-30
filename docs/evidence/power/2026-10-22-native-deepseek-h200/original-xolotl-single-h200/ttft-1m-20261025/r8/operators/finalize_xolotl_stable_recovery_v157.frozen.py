"""Read-only verification after the one VIP switch, using actual Windows requests."""
import json, time, urllib.request, uuid
from pathlib import Path
from xolotl_serving_common import BASE, observe, jump

ip = BASE / 'xolotl-stable-recovery-v157-promotion.json'
record = json.loads(ip.read_text())
r = json.loads((BASE / 'xolotl-stable-service-v157-intent.json').read_text())
if record['phase'] not in ['switched', 'windows_validation_pending'] or record['new_allocation_uid'] != r['allocation_uid']:
    raise RuntimeError('only the existing V58 switch can be finalized')
observation = observe()
if observation['job_uid'] != r['allocation_uid'] or observation['phase'] != 'Running':
    raise RuntimeError('explicit V58 serving alias differs')
code = '''import json,shlex,subprocess
r=@R@
p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl','get','service','aster-xolotl-api-v3pool-20261024','-n','ailab-hrprdduanyan','-o','json'])],capture_output=True,text=True,check=True,timeout=15)
s=json.loads(p.stdout)
if s['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or s['spec']['selector']!={'rjob.brainpp.cn/rjob-name':r['allocation_job']} or s['metadata']['annotations']['aster.serving/allocation-uid']!=r['allocation_uid'] or s['metadata']['annotations']['aster.serving/lease-ends-unix']!=str(r['plan']['lease_ends_unix']):raise RuntimeError('already-switched V58 service differs')
print(json.dumps(dict(uid=s['metadata']['uid'],resource_version=s['metadata']['resourceVersion'],selector=s['spec']['selector'],load_balancer=s['status']['loadBalancer'])))
'''.replace('@R@', repr(r))
record['post_switch_service'] = jump(code)
key = Path(r['credential_file']).read_text().strip()
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
request = urllib.request.Request('http://10.102.93.68/health', headers={'Authorization': 'Bearer ' + key, 'Connection': 'close'})
with opener.open(request, timeout=10) as response:
    health = json.load(response)
    status = response.status
if status != 200 or not health.get('loaded') or health.get('max_context') != 1048576:
    raise RuntimeError('existing switched V58 health has not propagated; observe without repeating the mutation')
control_text='Public backend verification nonce '+uuid.uuid4().hex+'. Ignore filler. Filler:'+(' cat'*300)+'\nFinal question: Calculate 17 + 25. Reply with only the integer answer.'
prime_code="""import json,urllib.request
from pathlib import Path
r=@R@;text=@TEXT@;key=(Path(r['root'])/'credentials/api-key.txt').read_text().strip()
payload=dict(model=r['plan']['model_name'],messages=[dict(role='user',content=text)],max_tokens=16,temperature=0,reasoning_effort='none',stream=False)
request=urllib.request.Request('http://'+r['pod_ip']+':30003/v1/chat/completions',data=json.dumps(payload).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request,timeout=20) as response:out=json.load(response)
if out['choices'][0]['message']['content'].strip()!='42' or out['usage']['prompt_tokens']<256:raise RuntimeError('exact target-pod public cache priming failed')
print(json.dumps(dict(content='42',usage=out['usage'],direct_target_pod=r['pod_ip'])))
""".replace('@R@',repr(r)).replace('@TEXT@',repr(control_text))
primed=jump(prime_code)
payload = dict(model=r['plan']['model_name'], messages=[dict(role='user', content=control_text)],
               max_tokens=16, temperature=0, reasoning_effort='none', stream=True, stream_options=dict(include_usage=True))
request = urllib.request.Request('http://10.102.93.68/v1/chat/completions', data=json.dumps(payload).encode(),
                                 headers={'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json', 'Connection': 'close'})
start = time.monotonic()
first = None
text = ''
finish = usage = None
done = False
with opener.open(request, timeout=30) as response:
    for line in response:
        if not line.startswith(b'data:'):
            continue
        value = line[5:].strip()
        if value == b'[DONE]':
            done = True
            break
        data = json.loads(value)
        for choice in data.get('choices', []):
            part = choice.get('delta', {}).get('content') or ''
            if part:
                first = first or time.monotonic()
                text += part
            finish = choice.get('finish_reason') or finish
        usage = data.get('usage') or usage
if text.strip() != '42' or not done or not finish or usage is None or first is None:
    raise RuntimeError('direct Windows V58 streaming generation did not finish correctly')
if usage['prompt_tokens']!=primed['usage']['prompt_tokens'] or usage.get('prompt_tokens_details',{}).get('cached_tokens',0)<primed['usage']['prompt_tokens']-8:
    raise RuntimeError('Windows VIP request did not reuse the unique public prompt primed only on the new pod')
record.update(endpoint_object_read_permitted=False,endpoint_read_failure_previously_observed='Forbidden: endpoints resource is not allowed for the cluster user',backend_verified_by_unique_nonce_and_kv_reuse=True,target_pod_prime=primed)
record.update(phase='promoted', finished_unix=time.time(), windows_forwarding=False,
              windows_direct_health=dict(status=status, loaded=True, max_context=health['max_context']),
              windows_direct_generation=dict(content='42', done=True, finish_reason=finish, usage=usage,
                                             ttft_seconds=first-start, seconds=time.monotonic()-start),
              previous_card_already_terminal=True)
ip.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase='promoted', base_url=record['base_url'], same_existing_key=True,
                      actual_prompt_tokens_qualified=132097, full_1m_length_qualified=False,
                      windows_direct_generation=record['windows_direct_generation'])))
