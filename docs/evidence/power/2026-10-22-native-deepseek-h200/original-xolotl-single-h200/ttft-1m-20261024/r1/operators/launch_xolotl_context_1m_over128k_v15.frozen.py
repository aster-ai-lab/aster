"""Launch a bounded cluster-side 35,700-token API qualification client."""
import base64,hashlib,json,time
from xolotl_trial_common import BASE,jump,observe
ip=BASE/'xolotl-context-1m-over128k-v15-intent.json'
if ip.exists():raise RuntimeError('long-context client intent exists; inspect it')
r=json.loads((BASE/'xolotl-ttft-shadow-v15-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid'] or o['phase']!='Running':raise RuntimeError('shadow allocation changed')
root=r['root']+'/qualification-over128k-v15'
plan=dict(base_url='http://'+r['pod_ip']+':30003/v1',service_root=r['root'],context_tokens=1048576)
payload={'request-worker.py':(BASE/'xolotl_context_qos_remainder_worker_v15.frozen.py').read_bytes(),'request-plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
rec=dict(unix=time.time(),phase='prepared',root=root,allocation_uid=r['allocation_uid'],public_fixture_only=True,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()]);ip.write_text(json.dumps(rec,indent=2)+'\n')
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);service=root.parent
q=json.loads((service/'service-longctx-v1-result.json').read_text())
if service.resolve()!=service or service.stat().st_uid!=10250 or root.exists() or q['phase']!='serving' or q['context_tokens']!=1048576:raise RuntimeError('long-context service must be ready')
prior=json.loads((service/'qualification-ttft-v15/request-result.json').read_text())
if prior['phase']!='passed':raise RuntimeError('TTFT qualification must complete before the131073-token regression')
root.mkdir(mode=0o700);data=@DATA@
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('long-context client frozen input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'request-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','1100','python3','-u',str(root/'request-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=1100)
(root/'request-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@',repr(rec)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
compile(code,'long-context-client-bootstrap','exec');rec['phase']='launch_requested';ip.write_text(json.dumps(rec,indent=2)+'\n');rec['process']=jump(code);rec['phase']='launched';ip.write_text(json.dumps(rec,indent=2)+'\n');print(json.dumps(dict(phase=rec['phase'],root=root,public_fixture_only=True)))
