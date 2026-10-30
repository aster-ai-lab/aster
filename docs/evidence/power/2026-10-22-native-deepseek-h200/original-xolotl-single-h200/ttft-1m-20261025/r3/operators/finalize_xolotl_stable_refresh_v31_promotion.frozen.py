"""Read-only verification after the one VIP switch, using actual Windows requests."""
import json, time, urllib.request
from pathlib import Path
from xolotl_serving_common import BASE, observe, jump

ip = BASE / 'xolotl-stable-refresh-v31-promotion.json'
record = json.loads(ip.read_text())
r = json.loads((BASE / 'xolotl-stable-service-v31-intent.json').read_text())
if record['phase'] not in ['switched', 'windows_validation_pending'] or record['new_allocation_uid'] != r['allocation_uid']:
    raise RuntimeError('only the existing V15 switch can be finalized')
observation = observe()
if observation['job_uid'] != r['allocation_uid'] or observation['phase'] != 'Running':
    raise RuntimeError('explicit V15 serving alias differs')
code = '''import json,shlex,subprocess
r=@R@
p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl','get','service','aster-xolotl-api-v3pool-20261024','-n','ailab-hrprdduanyan','-o','json'])],capture_output=True,text=True,check=True,timeout=15)
s=json.loads(p.stdout)
if s['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or s['spec']['selector']!={'rjob.brainpp.cn/rjob-name':r['allocation_job']} or s['metadata']['annotations']['aster.serving/allocation-uid']!=r['allocation_uid'] or s['metadata']['annotations']['aster.serving/lease-ends-unix']!=str(r['plan']['lease_ends_unix']):raise RuntimeError('already-switched V15 service differs')
p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl','get','endpoints','aster-xolotl-api-v3pool-20261024','-n','ailab-hrprdduanyan','-o','json'])],capture_output=True,text=True,check=True,timeout=15)
e=json.loads(p.stdout);ips=sorted({address['ip'] for subset in e.get('subsets',[]) for address in subset.get('addresses',[])})
if ips!=[r['pod_ip']]:raise RuntimeError('VIP ready endpoints have not converged to the exact new pod; preserve switch and reobserve')
print(json.dumps(dict(uid=s['metadata']['uid'],resource_version=s['metadata']['resourceVersion'],selector=s['spec']['selector'],load_balancer=s['status']['loadBalancer'],ready_endpoint_ips=ips)))
'''.replace('@R@', repr(r))
record['post_switch_service'] = jump(code)
key = Path(r['credential_file']).read_text().strip()
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
request = urllib.request.Request('http://10.102.93.68/health', headers={'Authorization': 'Bearer ' + key, 'Connection': 'close'})
with opener.open(request, timeout=10) as response:
    health = json.load(response)
    status = response.status
if status != 200 or not health.get('loaded') or health.get('max_context') != 1048576:
    raise RuntimeError('existing switched V15 health has not propagated; observe without repeating the mutation')
payload = dict(model=r['plan']['model_name'], messages=[dict(role='user', content='Calculate 17 + 25. Reply with only the integer answer.')],
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
    raise RuntimeError('direct Windows V15 streaming generation did not finish correctly')
record.update(phase='promoted', finished_unix=time.time(), windows_forwarding=False,
              windows_direct_health=dict(status=status, loaded=True, max_context=health['max_context']),
              windows_direct_generation=dict(content='42', done=True, finish_reason=finish, usage=usage,
                                             ttft_seconds=first-start, seconds=time.monotonic()-start),
              old_card_pending_idle_drain=True)
ip.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase='promoted', base_url=record['base_url'], same_existing_key=True,
                      actual_prompt_tokens_qualified=132097, full_1m_length_qualified=False,
                      windows_direct_generation=record['windows_direct_generation'])))
