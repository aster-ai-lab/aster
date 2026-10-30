"""Fresh exact runtime/VIP proof, stream recovery, and current-service aliases after the verified repair."""
import hashlib,importlib.util,json,sys,time,urllib.request,uuid
from xolotl_recovery_common_v217 import BASE,observe,owned

stage=json.loads((BASE/'xolotl-formal-tool-repair-v284-intent.json').read_text())
old=json.loads((BASE/'xolotl-stable-service-v279-intent.json').read_text())
probes=json.loads((BASE/'xolotl-formal-tool-repair-v286-live-probe.json').read_text())
client=json.loads((BASE/'workbuddy-read-repair-e2e-v287-result.json').read_text())
if probes['phase']!='passed' or client['phase']!='passed' or not client['native_tool_roundtrip_passed']:raise RuntimeError('actual fixed API and installed-client successful-tool proof required')
repeated=json.loads((BASE/'workbuddy-repeated-search-e2e-v287-result.json').read_text())
if repeated['phase']!='passed' or repeated['search_calls']!=3 or repeated['reply'].strip()!='COMPLETED=3':raise RuntimeError('actual installed-client repeated-call stop and truthful execution count required')
o=observe()
if o['phase']!='Running' or o['job_uid']!=stage['allocation_uid']:raise RuntimeError('fresh exact formal allocation changed')
runtime=owned('''import hashlib,json,os,time
from pathlib import Path
r=@R@;root=Path(r['root']);s=json.loads((root/'service-longctx-v1-result.json').read_text())
if s['phase']!='serving' or s['gpu_uuid']!=r['gpu_uuid'] or s['binary_sha256']!=r['native_sha256']:raise RuntimeError('repaired formal instance is not serving the exact qualified native')
api=s['api_process'];p=Path('/proc/'+str(api['pid']));stat=(p/'stat').read_text().split(') ',1)[1].split()
if p.stat().st_uid!=10250 or int(stat[19])!=api['start_ticks'] or stat[0]=='Z' or str(root/'source/tools/v41/serve/serve_ds41.py').encode() not in (p/'cmdline').read_bytes():raise RuntimeError('exact repaired API process differs')
plan=json.loads((root/'service-plan.json').read_bytes());found=[]
for p in Path('/proc').iterdir():
 if not p.name.isdecimal():continue
 try:
  if p.stat().st_uid!=10250 or os.readlink(p/'exe')!=plan['binary']:continue
  stat=(p/'stat').read_text().split(') ',1)[1].split()
  if stat[0]=='Z':continue
  found.append(dict(pid=int(p.name),start_ticks=int(stat[19]),parent_pid=int(stat[1]),namespace='formal pod'))
 except (FileNotFoundError,ProcessLookupError,PermissionError):continue
if len(found)!=1 or found[0]['parent_pid']!=api['pid']:raise RuntimeError('exact repaired native must be the only owned child')
source=[]
for name in ['source/serve/frontend.py','source/tools/v41/serve/serve_ds41.py','source/tools/v41/serve/aster_tool_roundtrip.py']:
 p=root/name;b=p.read_bytes();source.append(dict(file=name,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
print(json.dumps(dict(unix=time.time(),native=found[0],api=api,supervisor=json.loads((root/'service-process.json').read_text()),source=source,native_binary_sha256=s['binary_sha256'],lease_ends_unix=s['lease_ends_unix'],context_tokens=s['context_tokens'])))
'''.replace('@R@',repr(stage)))
key=(BASE/'access-private-xolotl-9c996c5a434b/api-key.txt').read_text().strip()
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
spec=importlib.util.spec_from_file_location('aster_final_protocol_fixtures_v279',BASE/'test_tool_roundtrip_v268.py')
fixture=importlib.util.module_from_spec(spec);sys.modules[spec.name]=fixture;spec.loader.exec_module(fixture)
body=dict(model=old['plan']['model_name'],messages=fixture.history(3),tools=fixture.TOOLS,max_tokens=128,temperature=0,reasoning_effort='none',stream=True)
body['messages'][0]['content']='Use completed public fixture results. Reply exactly alpha=<value>;beta=<value>. Do not call tools again.'
request=urllib.request.Request('http://10.102.93.68/v1/chat/completions',data=json.dumps(body).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
content=[];calls=0;finish=[];done=False;size=0;start=time.monotonic()
with opener.open(request,timeout=60) as response:
 for line in response:
  size+=len(line)
  if size>1024**2:raise RuntimeError('bounded live recovery SSE exceeded')
  if not line.startswith(b'data: '):continue
  raw=line[6:].strip()
  if raw==b'[DONE]':done=True;continue
  item=json.loads(raw)
  if item.get('error'):raise RuntimeError('live bounded recovery SSE returned an error')
  for choice in item.get('choices',[]):
   delta=choice.get('delta',{});content.append(delta.get('content') or '');calls+=len(delta.get('tool_calls') or [])
   if choice.get('finish_reason'):finish.append(choice['finish_reason'])
answer=''.join(content)
if not done or calls or finish!=['stop'] or answer.strip()!='alpha=25;beta=17':raise RuntimeError('live recovered SSE must finish once without any tool call')
proof=dict(unix=time.time(),phase='verified_serving',same_vip=True,same_existing_key=True,base_url='http://10.102.93.68/v1',
 allocation_uid=stage['allocation_uid'],gpu_uuid=stage['gpu_uuid'],root=stage['root'],runtime=runtime,
 live_nonstream_protocol_tests=2,live_stream_recovery=dict(passed=True,done=True,finish='stop',exposed_tool_calls=0,reply=answer,seconds=time.monotonic()-start),
 installed_WorkBuddy_Read_tool_passed=True,installed_WorkBuddy_search_error_terminated=True,
 successful_authenticated_search_E2E_passed=False,WorkBuddy_GUI_E2E_passed=False,
    installed_WorkBuddy_repeated_search_stopped_at_three_and_reported_three=True,
 original_loop_root_cause_exclusively_proven=False,ID_loss_defect_fixed=True,three_identical_completed_rounds_guard_deployed=True,
 native_performance_candidate_promoted=False,goal_complete=False)
with (BASE/'xolotl-formal-tool-repair-v288-verification.json').open('x') as f:f.write(json.dumps(proof,indent=2)+'\n')
active=json.loads(json.dumps(old));active.update(unix=time.time(),root=stage['root'],phase='serving',process=runtime['supervisor'],
    source_public_root=old['root'],credential_source_root=old['root'],replacement_reason='Tool-result ID association repair and bounded identical-completed-round recovery',
    API_overlay='tool-adapter-v267r3',API_overlay_source=runtime['source'],verification='xolotl-formal-tool-repair-v288-verification.json',
    fresh_instance_successful_tool_roundtrip_passed=True,workbuddy_gui_end_to_end_passed=False,instance_full_1m_execution_tested=False)
for row in active['plan']['inputs']:
 if row['path'].startswith(old['root']+'/'):row['path']=stage['root']+row['path'][len(old['root']):]
for row in runtime['source']:
 path=stage['root']+'/'+row['file'];active['plan']['inputs']=[i for i in active['plan']['inputs'] if i['path']!=path]
 active['plan']['inputs'].append(dict(path=path,bytes=row['bytes'],sha256=row['sha256'],verify_small_sha256=True))
with (BASE/'xolotl-stable-service-v288-intent.json').open('x') as f:f.write(json.dumps(active,indent=2)+'\n')
with (BASE/'xolotl-current-serving-service.json.before-v288').open('xb') as f:f.write((BASE/'xolotl-current-serving-service.json').read_bytes())
with (BASE/'xolotl-current-serving-service.json').open('w') as f:f.write(json.dumps(dict(unix=time.time(),intent='xolotl-stable-service-v288-intent.json',root=stage['root'],allocation_uid=stage['allocation_uid'],verification='xolotl-formal-tool-repair-v288-verification.json'),indent=2)+'\n')
print(json.dumps({k:proof[k] for k in ['phase','base_url','same_existing_key','live_stream_recovery','installed_WorkBuddy_Read_tool_passed','successful_authenticated_search_E2E_passed','WorkBuddy_GUI_E2E_passed','native_performance_candidate_promoted','goal_complete']},ensure_ascii=False,indent=2))
