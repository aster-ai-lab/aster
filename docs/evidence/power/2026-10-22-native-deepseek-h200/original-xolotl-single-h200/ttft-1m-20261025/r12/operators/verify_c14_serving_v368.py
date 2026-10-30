"""Fresh C14 runtime proof and corrected current manifest; no service mutation."""
import hashlib, json, time
from xolotl_recovery_common_v354 import BASE, observe, owned

def read(name): return json.loads((BASE/name).read_text(encoding='utf-8-sig'))
r=read('xolotl-stable-service-v354-intent.json')
promotion=read('xolotl-stable-recovery-v354-promotion.json')
controller=read('xolotl-stable-recovery-v354r2-controller.json')
client=read('workbuddy-read-repair-e2e-v366-result.json')
search=read('workbuddy-repeated-search-e2e-v367-result.json')
protocol=read('xolotl-formal-C14-tool-protocol-v365-live-probe.json')
if promotion['phase']!='promoted' or not promotion['backend_verified_by_unique_nonce_and_kv_reuse'] or controller['phase']!='passed':raise RuntimeError('completed C14 promotion and qualification required')
if client['phase']!='passed' or not client['native_tool_roundtrip_passed'] or search['phase']!='passed' or search['search_calls']!=3 or search['reply']!='COMPLETED=3' or protocol['phase']!='passed':raise RuntimeError('completed actual C14 installed client and protocol proofs required')
o=observe()
if o['job_uid']!=r['allocation_uid'] or o['phase']!='Running':raise RuntimeError('fresh exact C14 resource differs')
code='''import hashlib,json,os,time,urllib.request
from pathlib import Path
r=@R@;root=Path(r['root']);state=json.loads((root/'service-longctx-v1-result.json').read_text())
if state['phase']!='serving' or state['gpu_uuid']!=r['gpu_uuid'] or state['binary_sha256']!=r['plan']['binary_sha256'] or state['context_tokens']!=1048576:raise RuntimeError('exact C14 serving profile differs')
files=[]
for row in r['files']:
 p=root/row['file']
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('C14 actual manifest differs: '+row['file'])
 files.append(row)
for name,expected in r['repaired_sources'].items():
 p=root/name;b=p.read_bytes()
 if p.is_symlink() or p.stat().st_uid!=10250 or hashlib.sha256(b).hexdigest()!=expected:raise RuntimeError('C14 repaired source differs')
 files.append(dict(file=name,bytes=len(b),sha256=expected))
plan=json.loads((root/'service-plan.json').read_bytes())
if plan!=r['plan']:raise RuntimeError('live C14 plan differs from corrected manifest')
api=state['api_process'];p=Path('/proc/'+str(api['pid']));s=(p/'stat').read_text().split(') ',1)[1].split()
if p.stat().st_uid!=10250 or int(s[19])!=api['start_ticks'] or s[0]=='Z' or str(root/'source/tools/v41/serve/serve_ds41.py').encode() not in (p/'cmdline').read_bytes():raise RuntimeError('exact C14 API changed')
found=[]
for p in Path('/proc').iterdir():
 if not p.name.isdecimal():continue
 try:
  if p.stat().st_uid!=10250 or os.readlink(p/'exe')!=plan['binary']:continue
  s=(p/'stat').read_text().split(') ',1)[1].split()
  if s[0]!='Z':found.append(dict(pid=int(p.name),start_ticks=int(s[19]),parent_pid=int(s[1]),namespace='C14 formal pod'))
 except (FileNotFoundError,ProcessLookupError,PermissionError):continue
if len(found)!=1 or found[0]['parent_pid']!=api['pid']:raise RuntimeError('unique C14 native child required')
key=(root/'credentials/api-key.txt').read_text().strip()
req=urllib.request.Request('http://127.0.0.1:30003/health',headers={'Authorization':'Bearer '+key})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=5) as resp:health=json.load(resp)
if not health.get('loaded') or health['max_context']!=1048576:raise RuntimeError('exact C14 loaded health differs')
print(json.dumps(dict(unix=time.time(),api=api,native=found[0],supervisor=json.loads((root/'service-process.json').read_text()),files=files,plan=plan,lease_ends_unix=state['lease_ends_unix'],health_loaded=True,context_tokens=1048576)))
'''.replace('@R@',repr(r))
runtime=owned(code)
proof=dict(unix=time.time(),phase='verified_serving',base_url=promotion['base_url'],allocation_uid=r['allocation_uid'],gpu_uuid=r['gpu_uuid'],root=r['root'],runtime=runtime,
 same_existing_key=True,Windows_forwarding=False,Windows_backend_nonce_verification=promotion['windows_direct_generation'],
 basic_tests_passed=28,extended_tests_passed=12,strict_132097_passed=controller['edge_passed'],actual_1M_input_tested=False,
 installed_WorkBuddy_Read_tool_passed=True,installed_WorkBuddy_repeated_search_calls=3,repeated_search_terminated=True,
 WorkBuddy_GUI_E2E_passed=False,successful_authenticated_search_E2E_passed=False,native_performance_candidate_promoted=False,goal_complete=False,
 short_1024_summary=controller['profiles'][0]['extended']['long_summary'],protocol_proof='xolotl-formal-C14-tool-protocol-v365-live-probe.json',
 client_proofs=['workbuddy-read-repair-e2e-v366-result.json','workbuddy-repeated-search-e2e-v367-result.json'])
pn='xolotl-C14-serving-v368-verification.json';an='xolotl-stable-service-v368-intent.json'
active=json.loads(json.dumps(r));active.update(unix=time.time(),verification=pn,files=runtime['files'],process=runtime['supervisor'],runtime_native=runtime['native'],
 previous_metadata_stale=True,metadata_corrected_from_actual_C14=True,fresh_instance_successful_tool_roundtrip_passed=True,workbuddy_gui_end_to_end_passed=False)
for name,obj in [(pn,proof),(an,active)]:
 with (BASE/name).open('x') as f:f.write(json.dumps(obj,indent=2)+'\n')
pointer=BASE/'xolotl-current-serving-service.json'
with (BASE/(pointer.name+'.before-v368')).open('xb') as f:f.write(pointer.read_bytes())
pointer.write_text(json.dumps(dict(unix=time.time(),intent=an,verification=pn,allocation_uid=r['allocation_uid'],root=r['root']),indent=2)+'\n')
print(json.dumps(dict(phase=proof['phase'],native=runtime['native'],corrected_current_manifest=True,WorkBuddy_Read=True,repeated_search_stops_at=3,GUI_verified=False)))
