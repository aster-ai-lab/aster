"""Live Windows -> unchanged cluster VIP, reverse parallel results and bounded repeated-round recovery."""
import hashlib,importlib.util,json,sys,time,urllib.request
from xolotl_trial_common import BASE

spec=importlib.util.spec_from_file_location('aster_live_protocol_fixtures_v286',BASE/'test_tool_roundtrip_v268.py')
fixture=importlib.util.module_from_spec(spec);sys.modules[spec.name]=fixture;spec.loader.exec_module(fixture)
key=(BASE/'access-private-xolotl-9c996c5a434b/api-key.txt').read_text().strip()
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
def request(route,body=None):
 req=urllib.request.Request('http://10.102.93.68'+route,data=json.dumps(body).encode() if body else None,
     headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
 with opener.open(req,timeout=90) as response:
  raw=response.read(1024**2+1)
  if len(raw)>1024**2:raise RuntimeError('bounded public live protocol probe exceeded')
  return json.loads(raw)
health=request('/health')
if health.get('loaded') is not True or health.get('max_context')!=1048576:raise RuntimeError('repaired formal service not ready')
out=dict(unix=time.time(),phase='running',base_url='http://10.102.93.68/v1',same_existing_key=True,public_fixture_only=True,tools_executed=False,tests=[])
for name,rounds in [('reverse_parallel_results',1),('three_identical_rounds_recovery',3)]:
 body=dict(model='deepseek-v4.1-flash-1017-xq',messages=fixture.history(rounds),tools=fixture.TOOLS,
     max_tokens=128,temperature=0,seed=1,reasoning_effort='none',stream=False)
 body['messages'][0]['content']='Use completed public fixture results. Reply exactly alpha=<value>;beta=<value>. Do not call tools again.'
 start=time.monotonic();q=request('/v1/chat/completions',body);choice=q['choices'][0];m=choice['message'];answer=m.get('content') or ''
 row=dict(name=name,seconds=time.monotonic()-start,answer=answer,finish_reason=choice['finish_reason'],usage=q.get('usage'),
     exposed_tool_calls=len(m.get('tool_calls') or []),exact_passed=answer.strip()=='alpha=25;beta=17' and not m.get('tool_calls'))
 out['tests'].append(row);print(json.dumps(row,ensure_ascii=False),flush=True)
out['phase']='passed' if all(row['exact_passed'] for row in out['tests']) else 'failed'
with (BASE/'xolotl-formal-tool-repair-v286-live-probe.json').open('x') as f:f.write(json.dumps(out,indent=2)+'\n')
if out['phase']!='passed':raise RuntimeError('actual repaired-model protocol acceptance failed')
print(json.dumps(dict(phase=out['phase'],tests=len(out['tests']))))
