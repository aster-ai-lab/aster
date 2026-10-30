"""Recover only an authoritatively unstarted staged bootstrap; preserve its original refusal."""
import json
import time
from xolotl_trial_common import BASE, observe, owned

intent=BASE/'xolotl-engram-gpfs-b-recovery-v71r1-intent.json'
if intent.exists():
    raise RuntimeError('exact recovery handle exists; inspect it')
service_path=BASE/'xolotl-engram-disk-v68-gpfs_b-intent.json'
raw=service_path.read_bytes()
service=json.loads(raw)
if service['phase']!='launch_outcome_requires_inspection' or 'process' in service:
    raise RuntimeError('recovery requires the exact unstarted failed bootstrap')
controller=json.loads((BASE/'xolotl-engram-comparison-v71-controller.json').read_text())
if controller['phase']!='requires_inspection' or controller['profile']!='gpfs_b' or len(controller['profiles'])!=2 or 'shadow context qualification deadline insufficient' not in controller['error']:
    raise RuntimeError('exact original refusal/finished two profiles differs')
for name in ['xolotl-engram-natural-v69-gpfs_b-intent.json','xolotl-engram-extended-v70-gpfs_b-intent.json']:
    if (BASE/name).exists():
        raise RuntimeError('a GPFS-B client already has a handle')
resource=observe()
if resource['job_uid']!=service['allocation_uid']:
    raise RuntimeError('exact recovery resource differs')
(BASE/'xolotl-engram-disk-v68-gpfs_b-bootstrap-refused.json').write_bytes(raw)
record=dict(unix=time.time(),phase='prepared',root=service['root'],allocation_uid=service['allocation_uid'],
            reason='Original bootstrap required1800s remaining before any child was created; this finite19-request comparison requires900s; original service lease deadline retained',
            original_refusal_retained=True,resource_deadline_extended=False)
code='''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;s=@S@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact staged recovery directory differs')
for name in ['service-process.json','service-longctx-v1-result.json','service-worker.log','engine.log']:
 if (root/name).exists():raise RuntimeError('staged bootstrap has runtime evidence; inspect same handle instead')
for row in s['files']:
 p=root/row['file']
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('exact staged bootstrap source differs')
p=root/'credentials/api-key.txt'
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>256 or hashlib.sha256(p.read_bytes()).hexdigest()!=s['plan']['credential_file_sha256']:raise RuntimeError('exact private existing credential differs')
proof=json.loads((root.parent/'resource-result.json').read_text())
if proof['phase']!='passed' or proof['leaseEndsUnix']!=s['plan']['lease_ends_unix']:raise RuntimeError('unchanged exact resource lease differs')
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if active:raise RuntimeError('recovery GPU must be idle before launching the proven unstarted task')
deadline=int(s['plan']['lease_ends_unix']-time.time()-10)
if deadline<900:raise RuntimeError('finite19-request recovery budget insufficient')
with (root/'service-worker.log').open('xb') as log:
 child=subprocess.Popen(['timeout','--signal=TERM','--kill-after=30',str(deadline),'python3','-u',str(root/'service-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
p=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),deadline_seconds=deadline,namespace='optimization pod')
(root/'service-process.json').write_text(json.dumps(p,indent=2)+chr(10));print(json.dumps(p))
'''.replace('@R@',repr(record)).replace('@S@',repr(service))
compile(code,'exact-unstarted-Engram-bootstrap-recovery-v71r1','exec')
with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:
    record.update(process=owned(code),phase='launched')
except BaseException:
    record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n')
service.update(process=record['process'],phase='launched',original_refusal_retained=True,recovery_intent=intent.name)
service_path.write_text(json.dumps(service,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process'],resource_deadline_extended=False),indent=2))
