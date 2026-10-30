"""Keep service expiry cleanup on the server; no Windows listener, forwarding, or resident helper."""
import base64,hashlib,json,time
from xolotl_serving_common import BASE,jump
ip=BASE/'xolotl-server-vip-cleanup-v15-intent.json'
if ip.exists():raise RuntimeError('server VIP cleanup intent exists; inspect it')
r=json.loads((BASE/'xolotl-stable-service-v15-intent.json').read_text());promotion=json.loads((BASE/'xolotl-ttft-v15-promotion.json').read_text());s=promotion['switch']['after'];root=r['root']+'/server-vip-cleanup-v15'
plan=dict(service_name=s['metadata']['name'],service_uid=s['metadata']['uid'],namespace=s['metadata']['namespace'],allocation_uid=r['allocation_uid'],lease_ends_unix=r['plan']['lease_ends_unix'])
payload={'cleanup-worker.py':(BASE/'xolotl_server_vip_cleanup_worker_v4.py').read_bytes(),'cleanup-plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
rec=dict(unix=time.time(),phase='prepared',root=root,plan=plan,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()]);ip.write_text(json.dumps(rec,indent=2)+'\n')
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('server-side cleanup ownership/exclusivity differs')
root.mkdir(mode=0o700);data=@DATA@
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('cleanup frozen input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
budget=int(r['plan']['lease_ends_unix']-time.time()+180)
with (root/'cleanup-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15',str(budget),'python3','-u',str(root/'cleanup-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
row=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=budget)
(root/'cleanup-process.json').write_text(json.dumps(row,indent=2)+'\\n');print(json.dumps(row))
'''.replace('@R@',repr(rec)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
compile(code,'server-side-VIP-expiry-cleanup-bootstrap','exec');rec['phase']='launch_requested';ip.write_text(json.dumps(rec,indent=2)+'\n');rec['process']=jump(code);rec['phase']='launched';ip.write_text(json.dumps(rec,indent=2)+'\n');print(json.dumps(dict(phase=rec['phase'],server_side=True,no_windows_forwarding=True,lease_ends_unix=plan['lease_ends_unix'])))
